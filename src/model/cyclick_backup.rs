#[derive(Debug)]
pub struct CyclicBackup {
    pub target_name: String,
    pub cron_schedule: String,
}
