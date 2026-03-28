use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::error::Error;
use std::io::{stdin};
use std::sync::{Arc, Mutex};
use midir::{Ignore, MidiInput};

use std::collections::HashMap;
use ringbuf::{traits::*,HeapRb};

// REFERENCES
//https://github.com/Boddlnagg/midir/blob/d7f7366ee68cfd4b6b4d5af03d8fe6611f2ef21b/examples/test_read_input.rs

// Data Definitions
struct MidiInfo {
    note: u8,
    freq: f32,
    vel: u8
}


fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run() -> Result<(),Box<dyn Error>> {
    let host = cpal::default_host();
    let device = host.default_output_device().expect("No output device");
    let config = device.default_output_config().unwrap();

    let sample_rate = config.sample_rate().0 as f32;
    let mut t = 0.0;

    let mut active_notes:HashMap<u8,f32> = HashMap::new();
    let rb = HeapRb::<MidiInfo>::new(12);
    let (mut q_in, mut q_out) = rb.split();

    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    // get input ports - FIXME: this currently just selects the first available input
    let in_ports = midi_in.ports();
    let in_port = match in_ports.len() {
        0 => return Err("No Midi input".into()),
        _ => &in_ports[0]
    };

    // make midi connection
    let _conn_in = midi_in.connect(
        in_port,
        "midir-read-input",
        move |_stamp, message, _| {
            // for now only proceed when we get regular messages
            if message.len() != 3 { return; }
            let m = message[1];
            let f = calc_freq_from_midi(m);
            let info = MidiInfo {
                note: m,
                freq: f,
                vel: message[2],
            };
            //let mut freq = freq.lock().unwrap();
            //*freq = f;
            let _ = q_in.try_push(info); // sus
        },
        ()
    )?;

    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _| {
            // empty ring buffer, put into hash table
            while !q_out.is_empty() {
                let mi = q_out.try_pop().unwrap();
                println!("mi.vel = {}", mi.vel);
                if mi.vel == 0 {
                    active_notes.remove(&mi.note);
                }
                else {
                    active_notes.insert(mi.note, mi.freq);
                }
            }
            //let freq = *freq_clone.lock().unwrap();
            write_data_stream(data, &active_notes, &mut t, sample_rate)
        },
        |err| eprintln!("Error: {}", err),
        None,
    ).unwrap();

    stream.play().unwrap();

    let mut input = String::new();
    input.clear();
    stdin().read_line(&mut input)?; // wait for next enter key press
    Ok(())
}


fn write_data_stream(data: &mut [f32], active_notes: &HashMap<u8,f32>, t: &mut f32, sample_rate: f32) {
    for sample in data.iter_mut() {
        *sample = 0.0;
        for freq in active_notes.values() {
            *sample += 0.5 * (2.0 * std::f32::consts::PI * freq * *t).sin();
        }
        *t += 1.0 / sample_rate;
        if *t >= 1.0 {
            *t -= 1.0;
        }
    }
}

// calculates fundamental frequency from midi value
fn calc_freq_from_midi(m : u8) -> f32 {
    2.0f32.powf((m as f32 - 69.0)/12.0) * 440.0
}


// TESTS
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_freq_from_midi() {
        assert_eq!(calc_freq_from_midi(69), 440.0); //A4 = 440hz
        assert!((calc_freq_from_midi(96) - 2093.0).abs() < 0.01); //C7 = 2093.0hz
    }
}

/*
use std::error::Error;
use std::io::{stdin, stdout, Write};

use midir::{Ignore, MidiInput};

fn main() {
    env_logger::init();
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut input = String::new();

    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    // Get an input port (read from console if multiple are available)
    let in_ports = midi_in.ports();
    let in_port = match in_ports.len() {
        0 => return Err("no input port found".into()),
        1 => {
            println!(
                "Choosing the only available input port: {}",
                midi_in.port_name(&in_ports[0]).unwrap()
            );
            &in_ports[0]
        }
        _ => {
            println!("\nAvailable input ports:");
            for (i, p) in in_ports.iter().enumerate() {
                println!("{}: {}", i, midi_in.port_name(p).unwrap());
            }
            print!("Please select input port: ");
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            in_ports
                .get(input.trim().parse::<usize>()?)
                .ok_or("invalid input port selected")?
        }
    };

    println!("\nOpening connection");
    let in_port_name = midi_in.port_name(in_port)?;

    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    let _conn_in = midi_in.connect(
        in_port,
        "midir-read-input",
        move |stamp, message, _| {
            println!("{}: {:?} (len = {})", stamp, message, message.len());
        },
        (),
    )?;

    println!(
        "Connection open, reading input from '{}' (press enter to exit) ...",
        in_port_name
    );

    input.clear();
    stdin().read_line(&mut input)?; // wait for next enter key press

    println!("Closing connection");
    Ok(())
}
*/