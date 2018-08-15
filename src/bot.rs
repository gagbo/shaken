use crate::irc::{Conn, Message, Prefix};
use crate::*;

pub struct Bot<'a> {
    conn: Conn,
    modules: Vec<&'a dyn Module>,
    // TODO this might have to be a closure
    inspect: Box<Fn(&Message, &Response) + 'a>,
}

impl<'a> Bot<'a> {
    /// just clone the connection
    pub fn new<C>(conn: C) -> Self
    where
        C: Into<Conn>,
    {
        let conn = conn.into();
        Self {
            conn,
            modules: vec![],
            inspect: Box::new(|_, _| {}),
        }
    }

    pub fn add(&mut self, m: &'a dyn Module) {
        self.modules.push(m)
    }

    pub fn set_inspect<F>(&mut self, f: F)
    where
        F: Fn(&Message, &Response) + 'a,
    {
        self.inspect = Box::new(f)
    }

    pub fn register(&self, nick: &str) {
        trace!("registering");

        // ircv3 stuff
        self.send("CAP REQ :twitch.tv/tags");
        self.send("CAP REQ :twitch.tv/membership");
        self.send("CAP REQ :twitch.tv/commands");

        self.send(&format!("PASS {}", env!("SHAKEN_TWITCH_PASSWORD")));
        self.send(&format!("NICK {}", &nick));

        // this would be needed for a real irc server
        // self.conn
        //     .write(&format!("USER {} * 8 :{}", "shaken_bot", "shaken_bot"));

        trace!("registered");
    }

    pub fn send(&self, data: &str) {
        self.conn.write(data)
    }

    pub fn run(&self) {
        trace!("starting run loop");
        while let Some(_) = self.step() {}
        trace!("ending the run loop");
    }

    fn try_make_request(msg: &Message) -> Option<Request> {
        let id = Self::add_user_from_msg(&msg);
        trace!("trying to make request for: `{}` {:?}", id, msg);
        match &msg.command[..] {
            "PRIVMSG" | "WHISPER" => {
                // sanity check
                if let Some(Prefix::User { .. }) = msg.prefix {
                    Request::try_parse(id, &msg.data)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn step(&self) -> Option<()> {
        let msg = Message::parse(self.conn.read()?.as_ref());
        let req = Self::try_make_request(&msg);

        let mut out = vec![];
        trace!("dispatching to modules");
        for module in &self.modules {
            match &msg.command[..] {
                "PRIVMSG" | "WHISPER" => {
                    // try commands first
                    if let Some(req) = &req {
                        out.push(module.command(req))
                    }
                    // then passives
                    out.push(module.passive(&msg));
                }
                // other message types go to the event handler
                _ => out.push(module.event(&msg)),
            }
        }
        trace!("done dispatching to modules");

        trace!("collecting to send");
        out.into_iter()
            .filter_map(|r| {
                r.and_then(|r| {
                    (self.inspect)(&msg, &r);
                    r.build(&msg)
                })
            }).inspect(|s| trace!("writing response: {}", s))
            .for_each(|m| self.send(&m));
        trace!("done sending");

        Some(())
    }

    fn add_user_from_msg(msg: &Message) -> i64 {
        macro_rules! expect {
            ($e:expr) => {
                $e.expect("user tags to be well formed")
            };
        };

        let user = match &msg.command[..] {
            "PRIVMSG"|"WHISPER" => Some(User {
                display: expect!(msg.tags.get_display()).to_string(),
                color: expect!(msg.tags.get_color()),
                userid: expect!(msg.tags.get_userid()),
            }),
            // this is /our/ user
            "GLOBALUSERSTATE" => Some(User {
                display: expect!(msg.tags.get_display()).to_string(),
                color: color::RGB::from("fc0fc0"),
                userid: expect!(msg.tags.get_userid()),
            }),
            _ => return -1,
        }.unwrap();

        trace!("trying to add user {:?}", user);
        let conn = crate::database::get_connection();
        let id = UserStore::create_user(&conn, &user);
        trace!("added user {:?}: {}", user, id);
        id
    }
}
