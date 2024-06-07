use anyhow::Result;

use crate::component::LocalComponent;

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

impl LocalComponent for Editor {
    async fn run(&mut self, mut cx: crate::component::Context) -> Result<()> {
        cx.call::<String, ()>("core.print", Some("editor online".to_string()))
            .await?;

        let result = cx.call::<(), String>("core.getinfo", None).await;
        println!("{result:?}");

        Ok(())
    }
}

enum Mode {
    Editing,
    Normal,
}
