use std::env;
use std::process;
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;

use sysinfo::{Pid, System};

fn main() {
    // get main programs location
    let current_exe = env::current_exe().expect("Failed to get current exe path");
    let bhaptics_dir = current_exe.parent().expect("couldn't back out of parent");
    let exe_dir = bhaptics_dir.parent().expect("Failed to get out of bhpatics");
    let main_dir = exe_dir.parent().expect("Couldn't back out of sidecars folder");
    let main_program = main_dir.join("vr-haptics-player.exe");

    // if our haptics setup isn't running
    println!("Checking if Haptics server is already running...");
    let mut system = System::new_all();
    if find_process("vr-haptics-player.exe", &mut system).is_none() &&
        find_process("Haptics.exe", &mut system).is_none() {
        println!("Haptics Server is not running. Launching new process...");
        
        let _status = process::Command::new(main_program)
            .output()
            .expect("Failed to launch main program");

        println!("PROGRAM EXITED");
        println!("STDERR: {:?}", _status.stderr);

        // if it already is running
    } else {
        println!("haptics server is already running. Attaching to its lifetime...");
        track_process_and_exit("vr-haptics-player.exe", &mut system);
    }

    println!("vrch-gui.exe has exited. Proxy shutting down.");
    exit(0);
}

fn track_process_and_exit(process_name: &str, system: &mut System) {
    // Refresh the list of processes.
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let tracked = find_process(process_name, system);
    let mut pid = Pid::from_u32(0);

    if pid.as_u32() == 0 {
        // Needs to be utilized to make compiler happy
        print!("");
    }
    
    match tracked {
        Some(p ) => {
            pid  = p
        },
        None => {
            panic!("No process with name: {}", process_name);
        }
    }

    loop {
        // Refresh the list of processes.
        system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&vec![pid]), true);
        // Check if any running process has a name that matches process_name.
        let process_found = system.processes()
            .values()
            .any(|process| process.pid() == pid);

        if !process_found {
            println!("Process '{}' has closed. Exiting host process.", process_name);
            exit(0);
        }

        // Sleep for a fixed duration before checking again.
        sleep(Duration::from_secs(2));
    }
}

fn find_process(process_name: &str, system: &mut System) -> Option<sysinfo::Pid> {
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    system.processes()
        .iter()
        .find(|(_, process)| process.name() == process_name)
        .map(|(pid, _)| *pid)
}