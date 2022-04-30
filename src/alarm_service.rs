use crate::speech_service::SpeechService;
use actix_web::web::Data;
use clokwerk::{Job, JobId, TimeUnits};
use log::*;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Alarm {
    // type doesn't matter. It's only for displaying
    pub time: String,
    pub repeat_delay: u32,
    pub repeat_count: usize,
    pub message: String,
    id: AlarmId,
}

pub struct AlarmService {
    scheduler: Arc<Mutex<clokwerk::AsyncScheduler>>,
    speech_service: Data<Mutex<SpeechService>>,
    alarms: Vec<Alarm>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AlarmId(JobId);

impl AlarmService {
    pub fn new(speech_service: Data<Mutex<SpeechService>>) -> Self {
        let scheduler = Arc::new(Mutex::new(clokwerk::AsyncScheduler::new()));
        let service = AlarmService {
            scheduler: scheduler.clone(),
            speech_service,
            alarms: vec![],
        };
        tokio::spawn(async move {
            loop {
                {
                    let mut scheduler = scheduler.lock().await;
                    scheduler.run_pending().await;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        });
        service
    }

    pub async fn add_alarm(
        &mut self,
        time: &str,
        repeat_delay: u32,
        repeat_count: usize,
        message: String,
    ) {
        let mut scheduler = self.scheduler.lock().await;
        let speech_service = self.speech_service.clone();
        let message_clone = message.clone();
        let job_id = scheduler
            .every(1.day())
            .at(time)
            .repeating_every(repeat_delay.minutes())
            .times(repeat_count)
            .run(move || {
                let speech_service = speech_service.clone();
                let message = message.to_owned();
                async move {
                    let mut speech_service = speech_service.lock().await;
                    info!("Alarm running");
                    speech_service.say_azure(&message).await.unwrap();
                }
            })
            .id();
        self.alarms.push(Alarm {
            time: time.to_owned(),
            repeat_count,
            repeat_delay,
            message: message_clone,
            id: AlarmId(job_id),
        });
    }

    pub fn alarms(&self) -> Vec<Alarm> {
        self.alarms.clone()
    }

    pub async fn remove(&mut self, alarm_id: AlarmId) {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.remove_job(alarm_id.0);
        self.alarms.retain(|alarm| alarm.id != alarm_id);
    }
}
