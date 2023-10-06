use std::process::{Child, Command};

// RAII (Resource Acquisition Is Initialized)
// When this struct is dropped, the child process is terminated
pub struct DriverProcess {
    child: Option<Child>,
    port: u16,
}

impl DriverProcess {
    pub fn new(command: &str, desired_port: u16) -> Self {
        // Specify port for geckodriver
        let child = Command::new(command)
            .arg("-p")
            .arg(desired_port.to_string())
            .spawn()
            .expect("Failed to start driver");

        DriverProcess {
            child: Some(child),
            port: desired_port,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for DriverProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill(); // Kill child process
            let _ = child.wait(); // Wait for the process to terminate
        }
    }
}
