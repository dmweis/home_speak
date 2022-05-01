#[cfg(feature = "hotreload")]
use actix_files::NamedFile;
use actix_web::{delete, get, post, web, App, HttpResponse, HttpServer, Responder};
use home_speak::{
    alarm_service::{Alarm, AlarmId, AlarmService},
    configuration::{get_configuration, AlarmConfig},
    speech_service::{AzureVoiceStyle, SpeechService, TtsService},
    template_messages::{generate_startup_message, human_current_time},
};
use log::*;
use simplelog::*;
use std::{path::PathBuf, str};
use structopt::StructOpt;
use tokio::sync::Mutex;

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
    port: web::Data<BoundPort>,
) -> impl Responder {
    let startup_message = generate_startup_message(port.0);
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
    let current_time = human_current_time();
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

#[post("/pause")]
async fn pause(speech_service: web::Data<Mutex<SpeechService>>) -> impl Responder {
    speech_service.lock().await.pause();
    HttpResponse::Ok().finish()
}

#[post("/resume")]
async fn resume(speech_service: web::Data<Mutex<SpeechService>>) -> impl Responder {
    speech_service.lock().await.resume();
    HttpResponse::Ok().finish()
}

#[post("/stop")]
async fn stop(speech_service: web::Data<Mutex<SpeechService>>) -> impl Responder {
    speech_service.lock().await.stop();
    HttpResponse::Ok().finish()
}

#[post("/set_volume/{volume}")]
async fn set_volume(
    speech_service: web::Data<Mutex<SpeechService>>,
    volume: web::Path<f32>,
) -> impl Responder {
    speech_service.lock().await.volume(volume.into_inner());
    HttpResponse::Ok().finish()
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
    let file = include_str!("../../static/index.html");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(file)
}

#[derive(Debug, Clone, Copy)]
struct BoundPort(u16);

#[derive(StructOpt, Debug)]
#[structopt(
    version = "0.1.0",
    author = "David M. Weis <dweis7@gmail.com>",
    about = "CLI tool for playing text to speech commands using Google text to speech cloud API"
)]
struct Opts {
    #[structopt(long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logging();
    let opts = Opts::from_args();

    let app_config = get_configuration(opts.config)?;

    let mut speech_service = SpeechService::new(
        app_config.tts_service_config.google_api_key,
        app_config.tts_service_config.azure_api_key,
        app_config.tts_service_config.cache_dir_path,
    )?;

    if !app_config.skip_intro {
        let startup_message = generate_startup_message(app_config.server_config.port);
        for message_part in startup_message {
            speech_service.say(&message_part, TtsService::Azure).await?;
        }
    }

    let speech_service = web::Data::new(tokio::sync::Mutex::new(speech_service));

    let alarm_service = web::Data::new(tokio::sync::Mutex::new(AlarmService::new(
        speech_service.clone(),
    )));

    if let Some(ref path) = app_config.alarm_config.save_file_path {
        if let Err(e) = alarm_service.lock().await.add_alarms_from_file(path).await {
            error!("Failed to read saved alarms from {} with error {}", path, e);
            return Err(anyhow::anyhow!("Failed to read saved alarms {}", e));
        }
    }

    let address = format!(
        "{}:{}",
        app_config.server_config.host, app_config.server_config.port
    );

    HttpServer::new(move || {
        let port = web::Data::new(BoundPort(app_config.server_config.port));
        let alarm_config = web::Data::new(app_config.alarm_config.clone());

        App::new()
            .service(intro_handler)
            .service(say_handler)
            .service(current_time_handler)
            .service(sample_azure_languages_handler)
            .service(say_angry_handler)
            .service(say_cheerful_handler)
            .service(say_sad_handler)
            .service(pause)
            .service(resume)
            .service(stop)
            .service(set_volume)
            .service(create_alarm)
            .service(list_alarms)
            .service(delete_alarm)
            .service(index)
            .app_data(speech_service.clone())
            .app_data(port)
            .app_data(alarm_service.clone())
            .app_data(alarm_config)
    })
    .bind(address)?
    .run()
    .await?;

    Ok(())
}

fn setup_logging() {
    if TermLogger::init(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .is_err()
    {
        eprintln!("Failed to create term logger");
        if SimpleLogger::init(LevelFilter::Info, Config::default()).is_err() {
            eprintln!("Failed to create simple logger");
        }
    }
}
