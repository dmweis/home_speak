use crate::{
    error::Result,
    speech_service::{AzureVoiceStyle, SpeechService},
    template_messages::{get_human_current_date_time, get_human_current_time},
};
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
    pub style: AzureVoiceStyle,
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
        style: AzureVoiceStyle,
    ) -> Result<()> {
        let mut scheduler = self.scheduler.lock().await;
        let speech_service = self.speech_service.clone();
        let message_clone = message.clone();
        let job_id = scheduler
            .every(1.day())
            .try_at(time)?
            .repeating_every(repeat_delay.minutes())
            .times(repeat_count)
            .run(move || {
                let speech_service = speech_service.clone();
                let message = message.to_owned();
                async move {
                    let current_time = get_human_current_time();
                    let current_date_time = get_human_current_date_time();
                    let processed_message = message
                        .replace("/time", &current_time)
                        .replace("/date", &current_date_time);
                    let mut speech_service = speech_service.lock().await;
                    info!("Alarm running");
                    speech_service
                        .say_azure_with_style(&processed_message, style)
                        .await
                        .unwrap();
                }
            })
            .id();
        self.alarms.push(Alarm {
            time: time.to_owned(),
            repeat_count,
            repeat_delay,
            message: message_clone,
            style,
            id: AlarmId(job_id),
        });
        // sort after insertion
        self.alarms.sort_by(|a, b| a.time.cmp(&b.time));
        Ok(())
    }

    pub fn alarms(&self) -> Vec<Alarm> {
        self.alarms.clone()
    }

    pub async fn remove(&mut self, alarm_id: AlarmId) {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.remove_job(alarm_id.0);
        self.alarms.retain(|alarm| alarm.id != alarm_id);
    }

    pub async fn save_alarms_to_file(&self, path: &str) -> Result<()> {
        let alarm_configs: Vec<SavedAlarm> = self.alarms.iter().map(SavedAlarm::from).collect();
        let config = SavedAlarmConfig {
            alarms: alarm_configs,
        };
        let json = serde_json::to_string_pretty(&config)?;
        tokio::fs::write(path, json.as_bytes()).await?;
        Ok(())
    }

    pub async fn add_alarms_from_file(&mut self, path: &str) -> Result<()> {
        let data = tokio::fs::read(path).await?;
        let config: SavedAlarmConfig = serde_json::from_slice(&data)?;
        for alarm in config.alarms {
            // this sorts after each insertion but who cares?
            self.add_alarm(
                &alarm.time,
                alarm.repeat_delay,
                alarm.repeat_count,
                alarm.message,
                alarm.style,
            )
            .await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedAlarm {
    pub time: String,
    pub repeat_delay: u32,
    pub repeat_count: usize,
    pub message: String,
    #[serde(default)]
    pub style: AzureVoiceStyle,
}

impl From<&Alarm> for SavedAlarm {
    fn from(alarm: &Alarm) -> Self {
        SavedAlarm {
            time: alarm.time.clone(),
            repeat_delay: alarm.repeat_delay,
            repeat_count: alarm.repeat_count,
            message: alarm.message.clone(),
            style: alarm.style,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedAlarmConfig {
    pub alarms: Vec<SavedAlarm>,
}
