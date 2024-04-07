use actix_web::{delete, get, post, put, web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool, Row};

const DB_URL: &str = "sqlite://sqlite.db";

pub struct AppState {
    db: SqlitePool,
}


#[tokio::main]
async fn main() -> std::io::Result<()> {
    if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
        println!("Creating database {}", DB_URL);
        match Sqlite::create_database(DB_URL).await {
            Ok(_) => println!("Create db success"),
            Err(error) => panic!("error: {}", error),
        }
    } else {
        println!("Database already exists");
    }

    let db = SqlitePool::connect(DB_URL).await.unwrap();
    let result = sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS "data" (
            "timestamp"	INTEGER UNIQUE NOT NULL,
            "calories"	NUMERIC NOT NULL DEFAULT 0,
            CONSTRAINT "cal_not_neg" CHECK(calories >= 0),
            CONSTRAINT "non_zero_timestamp" CHECK(timestamp != 0)
        );"#).execute(&db).await.unwrap();
    println!("Create user table result: {:?}", result);

    HttpServer::new(move || {
        App::new()
        .app_data(web::Data::new(AppState { db: db.clone() }))
        .service(get_health)
        .service(post_health)
        .service(delete_health)
    })
        .bind(("0.0.0.0", 8080))?
        .run()
        .await?;

    Ok(())
}

#[derive(sqlx::Decode, sqlx::FromRow, serde::Serialize, Deserialize, Debug)]
struct HealthData {
    calories: i64,
}

#[derive(sqlx::Decode, sqlx::FromRow, serde::Serialize, Deserialize, Debug)]
struct TimestampedHealthData {
    timestamp: i64,
    #[sqlx(flatten)]
    health_data: HealthData,
}

#[get("/health")]
async fn get_health(data: web::Data<AppState>,) -> impl Responder {
    let result = sqlx::query_as::<_, TimestampedHealthData>("SELECT * FROM data")
        .fetch_all(&data.db)
        .await
        .unwrap();

    web::Json(result)
}


#[put("/health/{timestamp}")]
async fn post_health(path: web::Path<(u32,)>, data: web::Data<AppState>, body: web::Json<HealthData>) -> impl Responder {
    let result = 
        sqlx::query("INSERT INTO data (timestamp, calories) VALUES (?, ?)")
        .bind(path.into_inner().0)
        .bind(body.calories)
        .execute(&data.db)
        .await;

    match result {
        Ok(_) => HttpResponse::Created().body("Success"),
        Err(error) => HttpResponse::BadRequest().body(format!("error: {}", error))
    }
}

#[delete("/health/{timestamp}")]
async fn delete_health(path: web::Path<(u32,)>, data: web::Data<AppState>) -> impl Responder {
    let result = sqlx::query("DELETE FROM data WHERE timestamp = ?")
        .bind(path.into_inner().0)
        .execute(&data.db)
        .await;

    match result {
        Ok(db_resp) => match db_resp.rows_affected() {
            0 => HttpResponse::NotFound().body("No data found"),
            _ => HttpResponse::Ok().body("Success")
        }
        Err(error) => HttpResponse::BadRequest().body(format!("error: {}", error))
    }
}