use crate::errorstack::to_error_stack;
use cron::error::Error as CronError;
use log::warn;
use std::error::Error as StdError;
use std::fmt::Debug;
use std::fmt::Display;
use std::mem;
use std::str::FromStr;
use std::sync::mpsc::channel;
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;

use chrono::Utc;
use cron::Schedule;
use log::{error, info};

pub trait Task: Send {
    fn task_name(&self) -> &str;

    fn run_task(&self);
}

struct Job {
    thread_handle: JoinHandle<()>,
    job_killer: Sender<()>,
}

pub struct CronJobs {
    jobs: Vec<Job>,
}

pub enum TaskError {
    IvalidCron(CronError),
}

impl Debug for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        to_error_stack(f, self)
    }
}

impl From<CronError> for TaskError {
    fn from(err: CronError) -> Self {
        TaskError::IvalidCron(err)
    }
}

impl StdError for TaskError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            TaskError::IvalidCron(error) => Some(error),
        }
    }
}

impl Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::IvalidCron(_) => write!(f, "Failed to parse cron expression."),
        }
    }
}

impl CronJobs {
    pub fn new() -> Self {
        Self { jobs: Vec::new() }
    }

    pub fn start<T>(&mut self, cron: String, task: T) -> Result<(), TaskError>
    where
        T: Task + 'static,
    {
        let schedule = Schedule::from_str(&cron)?;
        let (sender, receiver) = channel();
        let handle = thread::spawn(move || loop {
            let mut upcomming = schedule.upcoming(Utc);
            let next_task_date = match upcomming.next() {
                Some(next_task_date) => next_task_date,
                None => {
                    warn!("Provided cron '{cron}' will never fire. Exiting immediately...");
                    return;
                }
            };
            let now = Utc::now();
            let diff = next_task_date
                .signed_duration_since(now)
                .abs()
                .to_std()
                .unwrap();
            info!(
                "Next scheduled task named '{}' at {next_task_date} (in {:?})",
                task.task_name(),
                diff
            );
            match receiver.recv_timeout(diff) {
                Ok(()) => {
                    info!("Got signal to end the task, exiting...");
                    return;
                }
                Err(err) => {
                    match err {
                        RecvTimeoutError::Timeout => {
                            info!("Running scheduled task named '{}'", task.task_name());
                            task.run_task();
                        }
                        RecvTimeoutError::Disconnected => {
                            error!("Sender died before receiver, it's probably an implementation mistake");
                            return;
                        }
                    }
                }
            }
        });
        self.jobs.push(Job {
            thread_handle: handle,
            job_killer: sender,
        });
        Ok(())
    }

    pub fn stop(&mut self) {
        let jobs = mem::replace(&mut self.jobs, vec![]);
        for job in jobs {
            let _ = job.job_killer.send(());
            job.thread_handle.join().unwrap();
        }
    }
}

impl Drop for CronJobs {
    fn drop(&mut self) {
        self.stop()
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::{CronJobs, Task};

    struct NoopTask {}

    impl Task for NoopTask {
        fn run_task(&self) {}

        fn task_name(&self) -> &str {
            "noop"
        }
    }

    #[test]
    fn should_return_error_when_invalid_cron_is_passed() {
        let mut cron_jobs = CronJobs::new();
        let schedule_result = cron_jobs.start("invalid".to_string(), NoopTask {});
        assert!(schedule_result.is_err());
    }

    #[test]
    fn should_not_panic_when_cron_is_only_in_the_past() {
        let mut cron_jobs = CronJobs::new();
        let schedule_result = cron_jobs.start("* * * * * * 2010".to_string(), NoopTask {});
        assert!(schedule_result.is_ok());

        std::thread::sleep(Duration::from_millis(10));
        assert!(cron_jobs.jobs[0].thread_handle.is_finished());
    }
}
