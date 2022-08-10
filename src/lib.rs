use log::debug;
use std::future::Future;

mod bot;
mod network;
mod nostr;
mod utils;

pub use bot::{help_command, Command, Commands, Functor, BotInfo, Sender};
pub use network::Network;
pub use nostr::{format_reply, Event, EventNonSigned};

use bot::{Profile, SenderRaw};

pub type FunctorRaw<State> =
    dyn Fn(
        nostr::Event,
        State,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = nostr::EventNonSigned>>>;

pub type State<T> = std::sync::Arc<tokio::sync::Mutex<T>>;

pub use bot::FunctorType;

#[macro_export]
macro_rules! wrap {
    ($functor:expr) => {
        FunctorType::Basic(Box::new(|event, state| Box::pin($functor(event, state))))
    };
}

#[macro_export]
macro_rules! wrap_extra {
    ($functor:expr) => {
        FunctorType::Extra(Box::new(|event, state, text| {
            Box::pin($functor(event, state, text))
        }))
    };
}

pub struct Bot<State: Clone + Send + Sync> {
    keypair: secp256k1::KeyPair,
    relays: Vec<String>,
    network_type: network::Network,
    commands: Commands<State>,

    profile: Profile,

    sender: Sender, // TODO: Use Option
    streams: Option<Vec<network::Stream>>,
    to_spawn: Vec<Box<dyn std::future::Future<Output = ()> + Send + Unpin>>,
}

impl<State: Clone + Send + Sync + 'static> Bot<State> {
    pub fn new(
        keypair: secp256k1::KeyPair,
        relays: Vec<String>,
        network_type: network::Network,
    ) -> Self {
        Bot {
            keypair,
            relays,
            network_type,
            commands: std::sync::Arc::new(std::sync::Mutex::new(vec![])),
            profile: Profile::new(),

            sender: std::sync::Arc::new(tokio::sync::Mutex::new(SenderRaw { sinks: vec![] })),
            streams: None,
            to_spawn: vec![],
        }
    }

    pub fn sender(mut self, sender: Sender) -> Self {
        self.sender = sender;
        self
    }

    pub fn command(self, command: Command<State>) -> Self {
        self.commands.lock().unwrap().push(command);
        self
    }

    pub fn set_name(mut self, name: &str) -> Self {
        self.profile.name = Some(name.to_string());
        self
    }

    pub fn set_about(mut self, about: &str) -> Self {
        self.profile.about = Some(about.to_string());
        self
    }

    pub fn set_picture(mut self, picture_url: &str) -> Self {
        self.profile.picture_url = Some(picture_url.to_string());
        self
    }

    pub fn set_intro_message(mut self, message: &str) -> Self {
        self.profile.intro_message = Some(message.to_string());
        self
    }

    pub fn help(self) -> Self {
        self.commands
            .lock()
            .unwrap()
            .push(Command::new("!help", wrap_extra!(help_command)).desc("Show this help."));
        self
    }

    pub fn spawn(mut self, fut: impl Future<Output = ()> + Unpin + Send + 'static) -> Self {
        self.to_spawn.push(Box::new(fut));
        self
    }

    pub async fn connect(&mut self) {
        debug!("Connecting to relays.");
        let (sinks, streams) = network::try_connect(&self.relays, &self.network_type).await;
        assert!(!sinks.is_empty() && !streams.is_empty());
        // TODO: Check is sender isn't filled already
        *self.sender.lock().await = SenderRaw { sinks };
        self.streams = Some(streams);
    }

    pub async fn run(&mut self, state: State) {
        if let None = self.streams {
            debug!("Running run() but there is no connection yet. Connecting now.");
            self.connect().await;
        }

        self.really_run(state).await;
    }
}

pub fn new_sender() -> Sender {
    std::sync::Arc::new(tokio::sync::Mutex::new(SenderRaw{ sinks: vec![]}))
}


pub fn init_logger() {
    // let _start = std::time::Instant::now();
    env_logger::Builder::from_default_env()
        // .format(move |buf, rec| {
        // let t = start.elapsed().as_secs_f32();
        // writeln!(buf, "{:.03} [{}] - {}", t, rec.level(), rec.args())
        // })
        .init();
}

pub fn wrap_state<T>(gift: T) -> State<T> {
    std::sync::Arc::new(tokio::sync::Mutex::new(gift))
}
