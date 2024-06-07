use anyhow::Result;

use crate::component::Component;

pub struct Editor {
    mode: Mode,

    buffers: Vec<String>,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            buffers: Vec::new(),
        }
    }
}

impl Component for Editor {
    async fn run(&mut self, mut cx: crate::component::Context) -> Result<()> {
        cx.call::<String, ()>("core.print", Some("editor online".to_string()))
            .await;

        let result = cx.call::<(), String>("core.getinfo", None).await?;
        println!("{result:?}");

        cx.call::<(), ()>("core.quit", None).await?;

        Ok(())
    }
}

enum Mode {
    Editing,
    Normal,
}
