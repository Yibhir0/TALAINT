use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://talaint:talaint@localhost:5432/talaint".to_string());
    let pool = talaint_control_plane::create_pool(&database_url).await?;
    talaint_control_plane::run_migrations(&pool).await?;
    println!("migrations applied");
    Ok(())
}
