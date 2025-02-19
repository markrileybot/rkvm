use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{Context, Error};
use log::{error, LevelFilter};
use structopt::StructOpt;
use tokio::fs;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time;
use tokio_native_tls::native_tls::{Identity, TlsAcceptor};

use config::Config;
use input::{clipboard, Direction, Event, EventManager, Key, KeyKind};
use net::{self, Message, PROTOCOL_VERSION};

mod config;

#[derive(Clone, Debug)]
struct Client {
    name: String,
    sender: UnboundedSender<Message>,
}

async fn handle_connection<T>(
    mut stream: T,
    mut receiver: UnboundedReceiver<Message>,
    sender: UnboundedSender<Message>,
) -> Result<(), Error>
    where
        T: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        tokio::select! {
            out_message = time::timeout(net::MESSAGE_TIMEOUT / 2, receiver.recv()) => {
                let message = match out_message {
                    Ok(Some(message)) => message,
                    Ok(None) => return Ok(()),
                    Err(_) => Message::KeepAlive,
                };

                time::timeout(net::MESSAGE_TIMEOUT, net::write_message(&mut stream, &message))
                    .await
                    .context("Write timeout")??;
            }
            in_message = net::read_message(&mut stream) => {
                sender.send(in_message?)?;
            }
        }
    }
}

async fn run(
    listen_address: SocketAddr,
    switch_keys: &HashSet<Key>,
    kill_keys: &HashSet<Key>,
    identity_path: &Path,
    identity_password: &str,
) -> Result<Infallible, Error> {
    let identity = fs::read(identity_path)
        .await
        .context("Failed to read identity")?;
    let identity =
        Identity::from_pkcs12(&identity, identity_password).context("Failed to parse identity")?;
    let acceptor: tokio_native_tls::TlsAcceptor = TlsAcceptor::new(identity)
        .context("Failed to create TLS acceptor")
        .map(Into::into)?;
    let listener = TcpListener::bind(listen_address).await?;

    log::info!("Listening on {}", listen_address);

    let (client_sender, mut client_receiver) = mpsc::unbounded_channel();
    let (in_sender, mut in_receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let (stream, address) = match listener.accept().await {
                Ok(sa) => sa,
                Err(err) => {
                    let _ = client_sender.send(Err(err));
                    return;
                }
            };

            let mut stream = match acceptor.accept(stream).await {
                Ok(stream) => stream,
                Err(err) => {
                    log::error!("{}: TLS error: {}", address, err);
                    continue;
                }
            };

            if let Err(e) = net::write_version(&mut stream, PROTOCOL_VERSION).await {
                error!("{}: Failed to write version: {}", address, e);
                continue;
            }

            match net::read_version(&mut stream).await {
                Ok(version) => {
                    if version != PROTOCOL_VERSION {
                        error!("Incompatible protocol version (got {}, expecting {})", version, PROTOCOL_VERSION);
                        continue;
                    }
                }
                Err(e) => {
                    error!("{}: Failed to read version: {}", address, e);
                    continue;
                }
            }

            let client_name = match net::read_message(&mut stream).await {
                Ok(Message::Hello(name)) => name,
                Ok(message) => {
                    error!("{}: Failed to read name.  Read {:?}", address, message);
                    continue;
                }
                Err(e) => {
                    error!("{}: Failed to read name: {}", address, e);
                    continue;
                }
            };

            let (out_sender, out_receiver) = mpsc::unbounded_channel();
            if client_sender.send(Ok(Client {name: client_name.clone(), sender: out_sender})).is_err() {
                return;
            }

            let message_sender = in_sender.clone();
            tokio::spawn(async move {
                log::info!("{} {}: connected", client_name, address);
                let message = handle_connection(stream, out_receiver, message_sender)
                    .await
                    .err()
                    .map(|err| format!(" ({})", err))
                    .unwrap_or_else(String::new);
                log::info!("{} {}: disconnected{}", client_name, address, message);
            });
        }
    });

    let mut clients: Vec<Client> = Vec::new();
    let mut current = 0;
    let mut manager = EventManager::new().await?;
    let mut switch_key_states: HashMap<_, _> = switch_keys
        .iter()
        .map(|key| (key.clone(), false))
        .collect();
    let mut kill_key_states: HashMap<_, _> = kill_keys
        .iter()
        .map(|key| (key.clone(), false))
        .collect();
    loop {
        tokio::select! {
            message = in_receiver.recv() => {
                if let Some(message) = message {
                    match message {
                        Message::SetClipboardData(text) => {
                            if current == 0 {
                                clipboard::set_text(text);
                            } else {
                                let idx = current - 1;
                                if let Err(e) = clients[idx].sender.send(Message::SetClipboardData(text)) {
                                    log::warn!("{:?}", e);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            event = manager.read() => {
                let event = event?;
                if let Event::Key { direction, kind: KeyKind::Key(key) } = event {
                    if let Some(state) = switch_key_states.get_mut(&key) {
                        *state = direction == Direction::Down;
                    } else if let Some(state) = kill_key_states.get_mut(&key) {
                        *state = direction == Direction::Down;
                    }
                }

                // TODO: This won't work with multiple keys.
                if switch_key_states.iter().filter(|(_, state)| **state).count() == switch_key_states.len() {
                    for state in switch_key_states.values_mut() {
                        *state = false;
                    }

                    let previous = current;
                    current = (current + 1) % (clients.len() + 1);
                    log::info!("Switching to client {} from {}", current, previous);

                    if current == 0 {
                        manager.notify("I'm over here now!".to_string());
                    } else {
                        let idx = current - 1;
                        if let Err(e) = clients[idx].sender.send(Message::Notify("I'm over here now!".to_string())) {
                            log::warn!("{:?}", e);
                        } else {
                            manager.notify(format!("Switched to {}", clients[idx].name).to_string());
                            log::debug!("Notify client {}", current);
                        }
                    }

                    if previous == 0 {
                        if let Some(text) = clipboard::get_text() {
                            let idx = current - 1;
                            if let Err(e) = clients[idx].sender.send(Message::SetClipboardData(text)) {
                                log::warn!("{:?}", e);
                            }
                        }
                    } else {
                        let idx = previous - 1;
                        if let Err(e) = clients[idx].sender.send(Message::GetClipboardData) {
                            log::warn!("{:?}", e);
                        }
                    }
                    continue;
                } else if kill_key_states.iter().filter(|(_, state)| **state).count() == kill_key_states.len() {
                    for state in kill_key_states.values_mut() {
                        *state = false;
                    }
                    return Err(Error::msg("Kilt"));
                }

                if current != 0 {
                    let idx = current - 1;
                    if let Err(e) = clients[idx].sender.send(Message::Event(event)) {
                        log::warn!("{:?}.  Removing client {}", e, current);
                        clients.remove(idx);
                        current = 0;
                    } else {
                        log::debug!("Send client {} {:?}", current, event);
                        continue;
                    }
                }

                log::debug!("Send manager {:?}", event);
                manager.write(event).await?;
            }
            sender = client_receiver.recv() => {
                clients.push(sender.unwrap()?);
            }
        }
    }
}

#[derive(StructOpt)]
#[structopt(name = "rkvm-server", about = "The rkvm server application")]
struct Args {
    #[structopt(help = "Path to configuration file")]
    #[cfg_attr(
    target_os = "linux",
    structopt(default_value = "/etc/rkvm/server.toml")
    )]
    #[cfg_attr(
    target_os = "windows",
    structopt(default_value = "C:/rkvm/server.toml")
    )]
    config_path: PathBuf,
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .format_timestamp(None)
        .filter(None, LevelFilter::Info)
        .init();

    let args = Args::from_args();
    let config = match fs::read_to_string(&args.config_path).await {
        Ok(config) => config,
        Err(err) => {
            log::error!("Error loading config: {}", err);
            process::exit(1);
        }
    };

    let config: Config = match toml::from_str(&config) {
        Ok(config) => config,
        Err(err) => {
            log::error!("Error parsing config: {}", err);
            process::exit(1);
        }
    };

    tokio::select! {
        result = run(config.listen_address, &config.switch_keys, &config.kill_keys, &config.identity_path, &config.identity_password) => {
            if let Err(err) = result {
                log::error!("Error: {:#}", err);
                process::exit(1);
            }
        }
        result = tokio::signal::ctrl_c() => {
            if let Err(err) = result {
                log::error!("Error setting up signal handler: {}", err);
                process::exit(1);
            }

            log::info!("Exiting on signal");
        }
    }
}
