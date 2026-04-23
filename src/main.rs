use archipelago_rs::{self as ap, Connection, ConnectionOptions, Event};
use ustr::{Ustr};
pub trait MessageLogger {
    fn log_message(&self) -> ();
}

impl MessageLogger for &str {
    fn log_message(&self) -> () {
        println!("[{}] [{}] {}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("Time goes forwards").as_millis(), "Program", &self);
    }
}

impl MessageLogger for ap::Error {
    fn log_message(&self) -> () {
        println!("[{}] [{}] {:?}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("Time goes forwards").as_millis(), "Error", &self);
    }
}

impl MessageLogger for ap::Event {
    fn log_message(&self) -> () {
        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("Time goes forwards").as_millis();
        match &self {
            Event::Print(print) => println!("[{}] [{}] {}", timestamp, "Archipelago", print),
            Event::DeathLink { games: _, slots: _, tags: _, time: _, cause, source } => {
                if let Some(cause) = cause {
                    println!("[{}] [{}] {} died with message {}", timestamp, "Deathlink", source, cause);
                } else {
                    println!("[{}] [{}] {} died without a message", timestamp, "Deathlink", source)
                }
            },
            Event::KeyChanged { key, old_value, new_value, player } => {
                let player = player.clone().map_or(String::from("Unknown"), |v| String::from(&*v.name()));
                let key: &str = key;
                if key.contains("GiftBox") {
                    println!("[{}] [Gifting] {} changed giftbox {} from {:?} to content {:?}", timestamp, player, key, old_value, new_value)
                } else if key == "EnergyLink0" {
                    println!("[{}] [Energylink] {} changed energylink from {:?} to value {:?}", timestamp, player, old_value, new_value)
                } 
            },
            Event::ReceivedItems(_) => (),
            Event::Bounce { games: _, slots: _, tags, data} => {
                if let Some(tagset) = tags {
                    if tagset.contains(&Ustr::from("RingLink")) {
                        let data: Option<serde_json::Value> = data.clone();
                        let data: serde_json::Value = match data {
                            Some(value) => value.clone(),
                            None => return,
                        };
                        println!("[{}] [{}] UUID {} sent {} rings.", timestamp, "RingLink", data["source"], data["amount"]);   
                    }
                }
            },
            Event::Updated(_updated_fields) => (),
            Event::Connected => println!("[{}] [{}] {}", timestamp, "Program", "Connected Succesfully!"),
            Event::Error(_error) => (), // Errors are handled in the main event logic.
        }
    }
}

fn main() {
    let url = std::env::args().nth(1).expect("No URL given");
    let slot_name = std::env::args().nth(2).expect("No slot name given");
    let mut connection: Connection<()> = Connection::new(
        &*url,
        &*slot_name,
        None::<String>,
        ConnectionOptions::new().tags([ap::tags::TRACKER, ap::tags::TEXT_ONLY, ap::tags::DEATH_LINK, "RingLink"]),
    );
    let mut watching_keys = false;
    loop {
        let events = connection.update();
        if connection.is_connected() {
            if !watching_keys {
                let mut watched_keys = vec![format!("EnergyLink0")];
                for player in connection.client().expect("We literally just checked we're connected").players() {
                    let key = format!("GiftBox;0;{}", player.slot());
                    watched_keys.push(key);
                }
                println!("{:?}", watched_keys);
                match connection.client_mut().expect("We literally just checked we're connected").watch(watched_keys) {
                    Ok(()) => {watching_keys = true; println!("Watching those keys!")},
                    Err(error) => error.log_message(), 
                }
            }
            for event in events {
                event.log_message();
            }
        } else if connection.is_connecting() {
            "Connecting!".log_message();
        } else if connection.is_disconnected() {
            watching_keys = false;
            connection.err().log_message();
            "Attempting to reconnect in one minute".log_message();
            std::thread::sleep(std::time::Duration::from_secs(60));
            connection = Connection::new(
                &*url,
                &*slot_name,
                None::<String>,
                ConnectionOptions::new().tags([ap::tags::TRACKER, ap::tags::TEXT_ONLY]),
            )
        }
        std::thread::sleep(std::time::Duration::from_secs(1))
    }
}
