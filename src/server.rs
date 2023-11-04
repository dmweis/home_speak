use crate::{
    alarm_service::{Alarm, AlarmId, AlarmService},
    configuration::{AlarmConfig, AppConfig},
    speech_service::{AzureVoiceStyle, SpeechService, TtsService},
    template_messages::{get_human_current_time, TemplateEngine},
};
#[cfg(feature = "hotreload")]
use actix_files::NamedFile;
use actix_web::{delete, get, post, web, App, HttpResponse, HttpServer, Responder};
use log::*;
use std::{str, sync::Arc};
use tokio::sync::Mutex;

#[derive(serde::Deserialize)]
struct SayCommand {
    content: String,
    style: AzureVoiceStyle,
    #[serde(default)]
    template: bool,
}

#[post("/say")]
async fn say_json_handler(
    command: web::Json<SayCommand>,
    speech_service: web::Data<Mutex<SpeechService>>,
) -> impl Responder {
    let message = if command.template {
        TemplateEngine::template_substitute(&command.content)
    } else {
        command.content.clone()
    };
    match speech_service
        .lock()
        .await
        .say_azure_with_style(&message, command.style)
        .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            error!("Failed to call speech service {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/say")]
async fn say_handler(
    body: web::Bytes,
    speech_service: web::Data<Mutex<SpeechService>>,
) -> impl Responder {
    if let Ok(text) = str::from_utf8(&body) {
        match speech_service
            .lock()
            .await
            .say(text, TtsService::Azure)
            .await
        {
            Ok(_) => HttpResponse::Ok().finish(),
            Err(e) => {
                error!("Failed to call speech service {}", e);
                HttpResponse::InternalServerError().finish()
            }
        }
    } else {
        error!("Failed processing rest request");
        HttpResponse::BadRequest().finish()
    }
}

// simple way to do this with no body
#[post("/say_angry")]
async fn say_angry_handler(
    body: web::Bytes,
    speech_service: web::Data<Mutex<SpeechService>>,
) -> impl Responder {
    if let Ok(text) = str::from_utf8(&body) {
        if let Err(e) = speech_service
            .lock()
            .await
            .say_azure_with_style(text, AzureVoiceStyle::Angry)
            .await
        {
            error!("Failed to call speech service {}", e);
            HttpResponse::InternalServerError().finish()
        } else {
            HttpResponse::Ok().finish()
        }
    } else {
        error!("Failed processing request");
        HttpResponse::BadRequest().finish()
    }
}

#[post("/say_sad")]
async fn say_sad_handler(
    body: web::Bytes,
    speech_service: web::Data<Mutex<SpeechService>>,
) -> impl Responder {
    if let Ok(text) = str::from_utf8(&body) {
        if let Err(e) = speech_service
            .lock()
            .await
            .say_azure_with_style(text, AzureVoiceStyle::Sad)
            .await
        {
            error!("Failed to call speech service {}", e);
            HttpResponse::InternalServerError().finish()
        } else {
            HttpResponse::Ok().finish()
        }
    } else {
        error!("Failed processing request");
        HttpResponse::BadRequest().finish()
    }
}

#[post("/say_cheerful")]
async fn say_cheerful_handler(
    body: web::Bytes,
    speech_service: web::Data<Mutex<SpeechService>>,
) -> impl Responder {
    if let Ok(text) = str::from_utf8(&body) {
        if let Err(e) = speech_service
            .lock()
            .await
            .say_azure_with_style(text, AzureVoiceStyle::Cheerful)
            .await
        {
            error!("Failed to call speech service {}", e);
            HttpResponse::InternalServerError().finish()
        } else {
            HttpResponse::Ok().finish()
        }
    } else {
        error!("Failed processing request");
        HttpResponse::BadRequest().finish()
    }
}

#[post("/sample_azure_languages")]
async fn sample_azure_languages_handler(
    body: web::Bytes,
    speech_service: web::Data<Mutex<SpeechService>>,
) -> impl Responder {
    if let Ok(text) = str::from_utf8(&body) {
        if let Err(e) = speech_service
            .lock()
            .await
            .sample_azure_languages(text)
            .await
        {
            error!("Failed to call speech service {}", e);
            HttpResponse::InternalServerError().finish()
        } else {
            HttpResponse::Ok().finish()
        }
    } else {
        error!("Failed processing rest request");
        HttpResponse::BadRequest().finish()
    }
}

#[post("/intro")]
async fn intro_handler(
    speech_service: web::Data<Mutex<SpeechService>>,
    template_engine: web::Data<TemplateEngine>,
) -> impl Responder {
    let startup_message = template_engine.startup_message();
    let mut speech_service = speech_service.lock().await;
    for message_portion in startup_message {
        if let Err(e) = speech_service
            .say(&message_portion, TtsService::Azure)
            .await
        {
            error!("Failed to call speech service {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    }
    HttpResponse::Ok().finish()
}

#[post("/current_time")]
async fn current_time_handler(speech_service: web::Data<Mutex<SpeechService>>) -> impl Responder {
    let current_time = get_human_current_time();
    match speech_service
        .lock()
        .await
        .say(
            &format!("Current time is {}", current_time),
            TtsService::Azure,
        )
        .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            error!("Failed to call speech service {}", e);

            HttpResponse::Ok().finish()
        }
    }
}

#[derive(serde::Deserialize)]
struct AlarmData {
    message: String,
    time: String,
    #[serde(default)]
    repeat_delay: u32,
    #[serde(default)]
    repeat_count: usize,
    #[serde(default)]
    style: AzureVoiceStyle,
}

#[post("alarm")]
async fn create_alarm(
    alarm_service: web::Data<Mutex<AlarmService>>,
    alarm_config: web::Data<AlarmConfig>,
    settings: web::Json<AlarmData>,
) -> impl Responder {
    let mut alarm_service = alarm_service.lock().await;
    info!("Creating a new alarm for {}", settings.time);
    if let Err(e) = alarm_service
        .add_alarm(
            &settings.time,
            settings.repeat_delay,
            settings.repeat_count,
            settings.message.clone(),
            settings.style,
        )
        .await
    {
        error!("Failed to add alarm {}", e);
        return HttpResponse::BadRequest().finish();
    }
    if let Some(ref config_path) = alarm_config.save_file_path {
        if let Err(e) = alarm_service.save_alarms_to_file(config_path).await {
            error!("Error saving alarms {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    }
    HttpResponse::Ok().finish()
}

#[derive(serde::Serialize)]
struct AlarmList {
    alarms: Vec<Alarm>,
}

#[get("alarm")]
async fn list_alarms(alarm_service: web::Data<Mutex<AlarmService>>) -> impl Responder {
    let alarm_service = alarm_service.lock().await;
    web::Json(AlarmList {
        alarms: alarm_service.alarms(),
    })
}

#[delete("alarm/{id}")]
async fn delete_alarm(
    alarm_service: web::Data<Mutex<AlarmService>>,
    alarm_config: web::Data<AlarmConfig>,
    id: web::Path<AlarmId>,
) -> impl Responder {
    let mut alarm_service = alarm_service.lock().await;
    info!("Deleting alarm {:?}", *id);
    alarm_service.remove(*id).await;
    if let Some(ref config_path) = alarm_config.save_file_path {
        if let Err(e) = alarm_service.save_alarms_to_file(config_path).await {
            error!("Error saving alarms {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    }
    HttpResponse::Ok().finish()
}

#[cfg(feature = "hotreload")]
#[get("/")]
async fn index() -> impl Responder {
    NamedFile::open_async("static/index.html").await
}

#[cfg(not(feature = "hotreload"))]
#[get("/")]
async fn index() -> impl Responder {
    let file = include_str!("../static/index.html");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(file)
}

pub async fn start_server(
    app_config: AppConfig,
    speech_service: Arc<Mutex<SpeechService>>,
    alarm_service: Arc<Mutex<AlarmService>>,
    template_engine: Arc<Mutex<TemplateEngine>>,
) -> anyhow::Result<()> {
    let address = format!(
        "{}:{}",
        app_config.server_config.host, app_config.server_config.port
    );

    let alarm_config = web::Data::new(app_config.alarm_config.clone());

    let speech_service = web::Data::from(speech_service);
    let template_engine = web::Data::from(template_engine);
    let alarm_service = web::Data::from(alarm_service);

    HttpServer::new(move || {
        App::new()
            .service(intro_handler)
            .service(say_json_handler)
            .service(say_handler)
            .service(current_time_handler)
            .service(sample_azure_languages_handler)
            .service(say_angry_handler)
            .service(say_cheerful_handler)
            .service(say_sad_handler)
            .service(create_alarm)
            .service(list_alarms)
            .service(delete_alarm)
            .service(index)
            .app_data(speech_service.clone())
            .app_data(template_engine.clone())
            .app_data(alarm_service.clone())
            .app_data(alarm_config.clone())
    })
    .bind(address)?
    .run()
    .await?;

    Ok(())
}
