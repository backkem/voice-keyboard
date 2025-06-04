use enigo::{Enigo, Keyboard, Settings};
use keyctl::{listen, Key};
use std::sync::{Arc, Mutex};

fn main() {
    println!("Listening for Quote key events...");

    // Create Enigo instance in a thread-safe wrapper
    let enigo = Arc::new(Mutex::new(
        Enigo::new(&Settings::default()).expect("Failed to create Enigo instance"),
    ));

    if let Err(error) = listen(Key::Quote, true, {
        let enigo = Arc::clone(&enigo);
        move |is_pressed| {
            if is_pressed {
                println!("Key down");
            } else {
                println!("Key up");

                std::thread::sleep(std::time::Duration::from_millis(100));

                // Handle text input in a thread-safe way
                if let Ok(mut enigo) = enigo.lock() {
                    if let Err(e) = enigo.text("Hello World! here is a lot of text  ❤️") {
                        eprintln!("Failed to send text: {}", e);
                    }
                } else {
                    eprintln!("Failed to acquire enigo lock");
                }
            }
        }
    }) {
        println!("Error: {:?}", error);
    }
}
