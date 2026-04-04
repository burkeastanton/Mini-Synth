use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::error::Error;
use std::io::{stdin};
use midir::{Ignore, MidiInput};

use std::collections::{HashMap, HashSet};
use ringbuf::{traits::*,HeapRb};

// REFERENCES
//https://github.com/Boddlnagg/midir/blob/d7f7366ee68cfd4b6b4d5af03d8fe6611f2ef21b/examples/test_read_input.rs

// Data Definitions
struct NoteInfo {
    note: u8,
    freq: f32,
    current_vel: f32,
    target_vel: f32,
    phase: f32,
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


    let mut active_notes:HashMap<u8,NoteInfo> = HashMap::new();
    let rb = HeapRb::<NoteInfo>::new(12);
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
            let info = NoteInfo {
                note: m,
                freq: f,
                target_vel: message[2] as f32 / 127.0,
                current_vel: 0.0,
                phase: 0.0,
            };
            let _ = q_in.try_push(info);
        },
        ()
    )?;

    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _| {
            // empty ring buffer, put into hash table
            while !q_out.is_empty() {
                let ni = q_out.try_pop().unwrap();
                println!("mi.vel = {}", ni.target_vel);
                if ni.target_vel == 0.0 {
                    // FIXME set to fade out
                    if active_notes.contains_key(&ni.note) {
                        let ni = active_notes.get_mut(&ni.note).unwrap();
                        ni.target_vel = 0.0;
                    }
                }
                else {
                    active_notes.insert(ni.note, ni);
                    println!("active notes.len() = {}", active_notes.len());
                }
            }
            write_data_stream(data, &mut active_notes, sample_rate)
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


fn write_data_stream(data: &mut [f32], active_notes: &mut HashMap<u8,NoteInfo>, sample_rate: f32) {

    // fill the buffer with samples
    for sample in data.iter_mut() {
        *sample = 0.0;
        let mut to_remove = HashSet::<u8>::new(); // FIXME this isn't performant probably
        // go through every active note
        for ni in active_notes.values_mut() {
            ni.current_vel += (ni.target_vel - ni.current_vel) / 1000.0; // handle clipping
            ni.phase += ni.freq / sample_rate; // update note timestep
            if ni.phase > 1.0 {ni.phase -= 1.0}; // loop phase back to stop floating point error
            // add computed note sample to whole sample
            //*sample += ni.current_vel * (2.0 * std::f32::consts::PI * ni.phase).sin();
            *sample += ni.current_vel * ni.phase; // saw wave

            // add note to removal set if
            if ni.target_vel == 0.0 && ni.current_vel < 0.0001 {
                to_remove.insert(ni.note);
            }
        }
        for note in to_remove {
            active_notes.remove(&note);
        }
    }
}

// calculates note frequency from midi value
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
