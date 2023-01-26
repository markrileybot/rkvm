use std::env;

use libc::{gid_t, uid_t};
use log::warn;
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
                            warn!("Failed to find group for {}.  Will not drop privileges.  {}", gid, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to parse gid from {}.  Will not drop privileges.  {}", gid, e);
                }
            }
        }
        Err(e) => {
            warn!("Failed to read sudo gid.  Will not drop privileges.  {}", e);
        }
    }
    None
}

fn get_sudo_user() -> Option<User> {
    match env::var("SUDO_UID") {
        Ok(uid) => {
            match uid.parse::<uid_t>() {
                Ok(uid) => {
                    match User::from_uid(Uid::from(uid)) {
                        Ok(user) => {
                            return user;
                        }
                        Err(e) => {
                            warn!("Failed to find user for {}.  Will not drop privileges.  {}", uid, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to parse uid from {}.  Will not drop privileges.  {}", uid, e);
                }
            }
        }
        Err(e) => {
            warn!("Failed to read sudo uid.  Will not drop privileges.  {}", e);
        }
    }
    None
}

pub(crate) fn drop_privileges() {
    if let Some(group) = get_sudo_group() {
        if let Some(user) = get_sudo_user() {
            warn!("Dropping to {:?}:{:?}", group, user);
            if let Err(e) = nix::unistd::setgid(group.gid) {
                warn!("Failed to set gid {}", e);
            } else {
                if let Err(e) = nix::unistd::setuid(user.uid) {
                    warn!("Failed to set uid {}", e);
                }
            }
        }
    }
}