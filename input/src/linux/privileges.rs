use std::env;

use libc::{gid_t, uid_t};
use log::{info, warn};
use nix::libc;
use nix::unistd::{Gid, Group, Uid, User};

fn get_sudo_group() -> Option<Group> {
    match env::var("SUDO_GID") {
        Ok(gid) => {
            match gid.parse::<gid_t>() {
                Ok(gid) => {
                    match Group::from_gid(Gid::from(gid)) {
                        Ok(group) => {
                            return group;
                        }
                        Err(e) => {
                            warn!("Failed to find group for {}.  {}", gid, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to parse gid from {}.  {}", gid, e);
                }
            }
        }
        Err(e) => {
            warn!("Failed to read sudo gid.  {}", e);
        }
    }
    None
}

fn get_user(uid: String) -> Option<User> {
    match uid.parse::<uid_t>() {
        Ok(uid) => {
            match User::from_uid(Uid::from(uid)) {
                Ok(user) => {
                    return user;
                }
                Err(e) => {
                    warn!("Failed to find user for {}.  {}", uid, e);
                }
            }
        }
        Err(e) => {
            warn!("Failed to parse uid from {}.  {}", uid, e);
        }
    }
    None
}

fn get_sudo_user() -> Option<User> {
    return match env::var("SUDO_UID") {
        Ok(uid) => {
            get_user(uid)
        }
        Err(e) => {
            info!("Failed to read sudo uid.  {}.  Trying polkit...", e);
            match env::var("PKEXEC_UID") {
                Ok(uid) => {
                    get_user(uid)
                }
                Err(e) => {
                    warn!("Failed to read polkit uid.  {}", e);
                    None
                }
            }
        }
    }
}

pub(crate) fn drop_privileges() {
    if let Some(group) = get_sudo_group() {
        info!("Dropping to group {:?}", group.name);
        if let Err(e) = nix::unistd::setgid(group.gid) {
            warn!("Failed to set gid {}", e);
        }
    }
    if let Some(user) = get_sudo_user() {
        info!("Dropping to user {:?}", user.name);
        if let Err(e) = nix::unistd::setuid(user.uid) {
            warn!("Failed to set uid {}", e);
        }
    }
}