use crate::prelude::*;

use std::sync::Arc;
use std::time::Instant;

use hashbrown::HashMap;
use log::*;

type Func<T> = fn(&mut T, &Request) -> Option<Response>; // this is for you, clippy.

pub struct CommandMap<T>(Arc<HashMap<&'static str, Func<T>>>);

impl<T> Clone for CommandMap<T> {
    fn clone(&self) -> Self {
        CommandMap(Arc::clone(&self.0))
    }
}

impl<T> CommandMap<T> {
    pub fn create<S>(
        namespace: S,
        commands: &[(&'static str, Func<T>)],
    ) -> Result<CommandMap<T>, ModuleError>
    where
        S: ToString,
    {
        let mut map = HashMap::new();
        let namespace = namespace.to_string();
        for (k, v) in commands.iter() {
            let cmd = CommandBuilder::command(*k)
                .namespace(namespace.clone())
                .build();

            if let Err(RegistryError::AlreadyExists) = Registry::register(&cmd) {
                warn!("{} already exists", cmd.name());
                return Err(ModuleError::CommandAlreadyExists);
            }
            map.insert(*k, *v);
        }
        Ok(CommandMap(Arc::new(map)))
    }

    // TODO get rid of these dumb allocations
    pub fn dispatch(&self, this: &mut T, req: &Request) -> Option<Response> {
        let mut maybes = vec![];
        for (cmd, func) in self.0.iter() {
            if let Some(req) = req.search(cmd) {
                maybes.push((cmd, func, req));
            }
        }

        if maybes.is_empty() {
            return None;
        }

        let first = maybes.remove(0);
        let (_, func, req) = maybes.iter().fold(first, |acc, (cmd, func, req)| {
            if cmd.len() < acc.0.len() {
                acc
            } else {
                (cmd, func, req.clone()) // hmm
            }
        });
        func(this, &req)
    }
}

pub trait Module: Send {
    fn handle(&mut self, rx: Receiver, tx: Sender) {
        // TODO handle panics here
        let mut resp = vec![];
        while let Ok(ev) = rx.recv() {
            let msg = match ev {
                Event::Message(msg, req) => {
                    match msg.command.as_str() {
                        "PRIVMSG" | "WHISPER" => {
                            if let Some(req) = req {
                                resp.push(self.command(&req));
                            }
                            resp.push(self.passive(&msg))
                        }
                        _ => resp.push(self.event(&msg)),
                    };
                    msg
                }
                Event::Tick(dt) => {
                    if let Some(resp) = self.tick(dt) {
                        let _ = tx.send((None, resp));
                    }
                    continue;
                }
                Event::Inspect(msg, resp) => {
                    self.inspect(&msg, &resp);
                    continue;
                }
            };

            for resp in resp.drain(..).filter_map(|s| s) {
                let _ = tx.send((Some(msg.clone()), resp));
            }
        }
    }

    fn command(&mut self, _req: &Request) -> Option<Response> {
        None
    }

    fn passive(&mut self, _msg: &irc::Message) -> Option<Response> {
        None
    }

    fn event(&mut self, _msg: &irc::Message) -> Option<Response> {
        None
    }

    fn tick(&mut self, _dt: Instant) -> Option<Response> {
        None
    }

    /// don't block in this or you'll probably break the tests
    // TODO: make this async
    fn inspect(&mut self, _msg: &irc::Message, _resp: &Response) {}
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Error {
    CommandAlreadyExists,
    CannotStart,
}
