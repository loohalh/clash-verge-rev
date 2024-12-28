use crate::core::clash_api::get_traffic_ws_url;
use crate::utils::help::format_bytes_speed;
use anyhow::Result;
use futures::Stream;
use image::{ImageBuffer, Rgba};
use imageproc::drawing::draw_text_mut;
use parking_lot::RwLock;
use rusttype::{Font, Scale};
use std::io::Cursor;
use std::sync::Arc;
use tokio_tungstenite::tungstenite::Message;
#[derive(Debug, Clone)]
pub struct SpeedRate {
    pub up_text: Arc<RwLock<Option<String>>>,
    pub down_text: Arc<RwLock<Option<String>>>,
}

impl SpeedRate {
    pub fn new() -> Self {
        Self {
            up_text: Arc::new(RwLock::new(None)),
            down_text: Arc::new(RwLock::new(None)),
        }
    }

    pub fn update_traffic(&self, up: u64, down: u64) {
        *self.up_text.write() = Some(format_bytes_speed(up));
        *self.down_text.write() = Some(format_bytes_speed(down));
    }

    /// 在图标上添加速率显示
    pub fn add_speed_text(icon: Vec<u8>, up_text: String, down_text: String) -> Result<Vec<u8>> {
        // 加载原始图标
        let img = image::load_from_memory(&icon)?;
        let (width, height) = (img.width(), img.height());

        let mut image = ImageBuffer::new((width as f32 * 4.0) as u32, height);

        // 将原图绘制在左侧
        image::imageops::replace(&mut image, &img, 0, 0);

        // 使用系统字体 (SF Mono)
        let font =
            Font::try_from_bytes(include_bytes!("../../../assets/fonts/SFCompact.ttf")).unwrap();

        // 调整渲染参数
        let color = Rgba([220u8, 220u8, 220u8, 230u8]); // 更淡的白色，略微透明
        let base_size = (height as f32 * 0.5) as f32; // 稍微减小字体大小
        let scale = Scale::uniform(base_size);

        // 计算文本宽度以实现右对齐
        let up_width = font
            .layout(up_text.as_ref(), scale, rusttype::Point { x: 0.0, y: 0.0 })
            .map(|g| g.position().x + g.unpositioned().h_metrics().advance_width)
            .last()
            .unwrap_or(0.0);

        let down_width = font
            .layout(
                down_text.as_ref(),
                scale,
                rusttype::Point { x: 0.0, y: 0.0 },
            )
            .map(|g| g.position().x + g.unpositioned().h_metrics().advance_width)
            .last()
            .unwrap_or(0.0);

        // 计算个文本的右对齐位置
        let right_margin = 8;
        let canvas_width = width * 4;

        // 为每个文本计算单独的x坐标以确保右对齐
        let up_x = canvas_width as f32 - up_width - right_margin as f32;
        let down_x = canvas_width as f32 - down_width - right_margin as f32;

        // 绘制上行速率
        draw_text_mut(
            &mut image,
            color,
            up_x as i32,
            1,
            scale,
            &font,
            up_text.as_ref(),
        );

        // 绘制下行速率
        draw_text_mut(
            &mut image,
            color,
            down_x as i32,
            height as i32 - (base_size as i32) - 1,
            scale,
            &font,
            down_text.as_ref(),
        );

        let mut bytes: Vec<u8> = Vec::new();
        let mut cursor = Cursor::new(&mut bytes);
        image.write_to(&mut cursor, image::ImageFormat::Png)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone)]
pub struct Traffic {
    pub up: u64,
    pub down: u64,
}
impl Traffic {
    pub async fn get_traffic_stream() -> Result<impl Stream<Item = Result<Traffic, anyhow::Error>>>
    {
        use futures::stream::{self, StreamExt};
        use std::time::Duration;

        let stream = Box::pin(
            stream::unfold((), |_| async {
                loop {
                    // 获取配置
                    let ws_url = get_traffic_ws_url().unwrap();

                    // 尝试建立连接
                    match tokio_tungstenite::connect_async(&ws_url).await {
                        Ok((ws_stream, _)) => {
                            println!("WebSocket connection established");
                            // 返回 websocket 流
                            return Some((
                                ws_stream.map(|msg| {
                                    msg.map_err(anyhow::Error::from).and_then(|msg: Message| {
                                        let data = msg.into_text()?;
                                        let json: serde_json::Value = serde_json::from_str(&data)?;
                                        Ok(Traffic {
                                            up: json["up"].as_u64().unwrap_or(0),
                                            down: json["down"].as_u64().unwrap_or(0),
                                        })
                                    })
                                }),
                                (),
                            ));
                        }
                        Err(e) => {
                            println!("WebSocket connection failed: {}", e);
                            // 连接失败后等待一段时间再重试
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    }
                }
            })
            .flatten(),
        );

        Ok(stream)
    }
}
