use anyhow::Result;

use editor::app::App;

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = App::new();

    app.run().await
}
