use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
use clokwerk::{Job, TimeUnits};
use home_speak::{
    configuration::get_configuration,
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
            .say_azure_with_feelings(text, AzureVoiceStyle::Angry)
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
            .say_azure_with_feelings(text, AzureVoiceStyle::Sad)
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
            .say_azure_with_feelings(text, AzureVoiceStyle::Cheerful)
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    // Test alarms
    // This sounds way to positive. I need it to say something long and annoying
    let mut scheduler = clokwerk::AsyncScheduler::new();
    let speech_service_clone = speech_service.clone();
    scheduler
        .every(1.day())
        .at("09:00")
        .repeating_every(3.minutes())
        .times(10)
        .run(move || {
            let speech_service_clone = speech_service_clone.clone();
            async move {
                let current_time = human_current_time();
                info!("Triggering alarm at {}", &current_time);
                let message = format!("Good morning! It's currently {} and it's time to get up and actually do something 
useful for once. Today is dentist day!", current_time);
                speech_service_clone
                    .lock()
                    .await
                    .say_azure_with_feelings(&message, AzureVoiceStyle::Cheerful)
                    .await
                    .unwrap();
                let bee_movie = "According to all known laws of aviation, there is no way a bee should 
be able to fly. It's wings are too small to get its fat little body off the ground. The bee, of course, flies 
anyway, because bees don't care what humans think is impossible.";
                speech_service_clone
                    .lock()
                    .await
                    .say_azure_with_feelings(bee_movie, AzureVoiceStyle::Angry)
                    .await
                    .unwrap();
            }
        });

    tokio::spawn(async move {
        loop {
            scheduler.run_pending().await;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });

    let address = format!(
        "{}:{}",
        app_config.server_config.host, app_config.server_config.port
    );

    HttpServer::new(move || {
        let port = web::Data::new(BoundPort(app_config.server_config.port));

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
            .app_data(speech_service.clone())
            .app_data(port)
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
