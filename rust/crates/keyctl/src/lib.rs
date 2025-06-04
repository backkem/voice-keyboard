use rdev::{grab as rdev_grab, listen as rdev_listen, Event, EventType, GrabError, ListenError};
use std::sync::{Arc, Mutex};

pub use rdev::Key;

#[derive(Debug)]
pub enum InputError {
    Listen(ListenError),
    Grab(GrabError),
}

/// Listen for hotkey events and call the callback with hotkey state (true = pressed, false = released)
/// De-duplicates repeated key press events when key is held down
///
/// # Arguments
///
/// * `hotkey` - The key to listen for
/// * `grab` - If true, uses grab mode (exclusive input capture), if false uses listen mode
/// * `callback` - Callback function called with boolean indicating key state
pub fn listen<T>(hotkey: Key, grab: bool, mut callback: T) -> Result<(), InputError>
where
    T: FnMut(bool) + 'static,
{
    let is_pressed = Arc::new(Mutex::new(false));

    if grab {
        let callback = Arc::new(Mutex::new(callback));
        let grab_handler = move |event: Event| -> Option<Event> {
            match event.event_type {
                EventType::KeyPress(key) => {
                    // println!("Key pressed: {:?}", key);
                    if key == hotkey {
                        let mut pressed = is_pressed.lock().unwrap();
                        if !*pressed {
                            *pressed = true;
                            if let Ok(mut cb) = callback.lock() {
                                cb(true);
                            }
                        }
                        None // Block the event
                    } else {
                        Some(event) // Pass through other events
                    }
                }
                EventType::KeyRelease(key) => {
                    if key == hotkey {
                        let mut pressed = is_pressed.lock().unwrap();
                        if *pressed {
                            *pressed = false;
                            if let Ok(mut cb) = callback.lock() {
                                cb(false);
                            }
                        }
                        None // Block the event
                    } else {
                        Some(event) // Pass through other events
                    }
                }
                _ => Some(event), // Pass through all other events
            }
        };
        rdev_grab(grab_handler).map_err(InputError::Grab)
    } else {
        let listen_handler = move |event: Event| match event.event_type {
            EventType::KeyPress(key) => {
                // println!("Key pressed: {:?}", key);
                if key == hotkey {
                    let mut pressed = is_pressed.lock().unwrap();
                    if !*pressed {
                        *pressed = true;
                        callback(true);
                    }
                }
            }
            EventType::KeyRelease(key) => {
                if key == hotkey {
                    let mut pressed = is_pressed.lock().unwrap();
                    if *pressed {
                        *pressed = false;
                        callback(false);
                    }
                }
            }
            _ => {}
        };
        rdev_listen(listen_handler).map_err(InputError::Listen)
    }
}
