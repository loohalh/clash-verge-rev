use crate::utils::dirs;
use crate::utils::help::format_bytes_speed;
use crate::{
    cmds,
    config::Config,
    feat, t,
    utils::resolve::{self, VERSION},
};
use anyhow::Result;
use futures::Stream;
use futures::StreamExt;
use image::{ImageBuffer, Rgba};
use imageproc::drawing::draw_text_mut;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use rusttype::{Font, Scale};
use std::io::Cursor;
use std::sync::Arc;
use tauri::AppHandle;
use tauri::{
    menu::CheckMenuItem,
    tray::{MouseButton, MouseButtonState, TrayIconEvent, TrayIconId},
};
use tauri::{
    menu::{MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    Wry,
};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

use super::handle;
pub struct Tray {
    pub speed_rate: Arc<RwLock<Option<SpeedRate>>>,
    shutdown_tx: Arc<RwLock<Option<broadcast::Sender<()>>>>,
}

impl Tray {
    pub fn global() -> &'static Tray {
        static TRAY: OnceCell<Tray> = OnceCell::new();

        TRAY.get_or_init(|| Tray {
            speed_rate: Arc::new(RwLock::new(None)),
            shutdown_tx: Arc::new(RwLock::new(None)),
        })
    }

    pub fn init(&self) -> Result<()> {
        let mut speed_rate = self.speed_rate.write();
        *speed_rate = Some(SpeedRate::new());

        Ok(())
    }

    pub fn create_systray(&self) -> Result<()> {
        let app_handle = handle::Handle::global().app_handle().unwrap();
        let tray_incon_id = TrayIconId::new("main");
        let tray = app_handle.tray_by_id(&tray_incon_id).unwrap();

        tray.on_tray_icon_event(|_, event| {
            let tray_event = { Config::verge().latest().tray_event.clone() };
            let tray_event: String = tray_event.unwrap_or("main_window".into());

            #[cfg(target_os = "macos")]
            if let TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Down,
                ..
            } = event
            {
                match tray_event.as_str() {
                    "system_proxy" => feat::toggle_system_proxy(),
                    "tun_mode" => feat::toggle_tun_mode(),
                    "main_window" => resolve::create_window(),
                    _ => {}
                }
            }

            #[cfg(not(target_os = "macos"))]
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Down,
                ..
            } = event
            {
                match tray_event.as_str() {
                    "system_proxy" => feat::toggle_system_proxy(),
                    "tun_mode" => feat::toggle_tun_mode(),
                    "main_window" => resolve::create_window(),
                    _ => {}
                }
            }
        });
        tray.on_menu_event(on_menu_event);
        Ok(())
    }

    /// 更新托盘菜单
    pub fn update_menu(&self) -> Result<()> {
        let app_handle = handle::Handle::global().app_handle().unwrap();
        let verge = Config::verge().latest().clone();
        let system_proxy = verge.enable_system_proxy.as_ref().unwrap_or(&false);
        let tun_mode = verge.enable_tun_mode.as_ref().unwrap_or(&false);

        let mode = {
            Config::clash()
                .latest()
                .0
                .get("mode")
                .map(|val| val.as_str().unwrap_or("rule"))
                .unwrap_or("rule")
                .to_owned()
        };

        let tray = app_handle.tray_by_id("main").unwrap();
        let _ = tray.set_menu(Some(create_tray_menu(
            &app_handle,
            Some(mode.as_str()),
            *system_proxy,
            *tun_mode,
        )?));
        Ok(())
    }

    /// 在图标上添��速率显示
    fn add_speed_text(icon: Vec<u8>, up_text: String, down_text: String) -> Result<Vec<u8>> {
        // 加载原始图标
        let img = image::load_from_memory(&icon)?;
        let (width, height) = (img.width(), img.height());

        let mut image = ImageBuffer::new((width as f32 * 4.0) as u32, height);

        // 将原图绘制在左侧
        image::imageops::replace(&mut image, &img, 0, 0);

        // 使用系统字体 (SF Mono)
        let font =
            Font::try_from_bytes(include_bytes!("../../assets/fonts/SFCompact.ttf")).unwrap();

        // 调整渲染参数
        let color = Rgba([220u8, 220u8, 220u8, 230u8]); // 更淡的白色，略微透明
        let base_size = (height as f32 * 0.5) as f32; // 稍微减小字体大小
        let scale = Scale::uniform(base_size);

        // 计算文本宽度以实现右对齐

        // 获取两个文本的宽度
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

        // 计算每个文本的右对齐位置
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

    /// 更新托盘图标
    pub fn update_icon(&self) -> Result<()> {
        let app_handle = handle::Handle::global().app_handle().unwrap();
        let verge = Config::verge().latest().clone();
        let system_proxy = verge.enable_system_proxy.as_ref().unwrap_or(&false);
        let tun_mode = verge.enable_tun_mode.as_ref().unwrap_or(&false);

        let common_tray_icon = verge.common_tray_icon.as_ref().unwrap_or(&false);
        let sysproxy_tray_icon = verge.sysproxy_tray_icon.as_ref().unwrap_or(&false);
        let tun_tray_icon = verge.tun_tray_icon.as_ref().unwrap_or(&false);

        let tray = app_handle.tray_by_id("main").unwrap();

        #[cfg(target_os = "macos")]
        let tray_icon = verge.tray_icon.clone().unwrap_or("monochrome".to_string());

        let icon_bytes = if *system_proxy && !*tun_mode {
            #[cfg(target_os = "macos")]
            let mut icon = match tray_icon.as_str() {
                "colorful" => include_bytes!("../../icons/tray-icon-sys.ico").to_vec(),
                _ => include_bytes!("../../icons/tray-icon-sys-mono.ico").to_vec(),
            };

            #[cfg(not(target_os = "macos"))]
            let mut icon = include_bytes!("../../icons/tray-icon-sys.ico").to_vec();
            if *sysproxy_tray_icon {
                let icon_dir_path = dirs::app_home_dir()?.join("icons");
                let png_path = icon_dir_path.join("sysproxy.png");
                let ico_path = icon_dir_path.join("sysproxy.ico");
                if ico_path.exists() {
                    icon = std::fs::read(ico_path).unwrap();
                } else if png_path.exists() {
                    icon = std::fs::read(png_path).unwrap();
                }
            }
            icon
        } else if *tun_mode {
            #[cfg(target_os = "macos")]
            let mut icon = match tray_icon.as_str() {
                "colorful" => include_bytes!("../../icons/tray-icon-tun.ico").to_vec(),
                _ => include_bytes!("../../icons/tray-icon-tun-mono.ico").to_vec(),
            };

            #[cfg(not(target_os = "macos"))]
            let mut icon = include_bytes!("../../icons/tray-icon-tun.ico").to_vec();
            if *tun_tray_icon {
                let icon_dir_path = dirs::app_home_dir()?.join("icons");
                let png_path = icon_dir_path.join("tun.png");
                let ico_path = icon_dir_path.join("tun.ico");
                if ico_path.exists() {
                    icon = std::fs::read(ico_path).unwrap();
                } else if png_path.exists() {
                    icon = std::fs::read(png_path).unwrap();
                }
            }
            icon
        } else {
            #[cfg(target_os = "macos")]
            let mut icon = match tray_icon.as_str() {
                "colorful" => include_bytes!("../../icons/tray-icon.ico").to_vec(),
                _ => include_bytes!("../../icons/tray-icon-mono.ico").to_vec(),
            };

            #[cfg(not(target_os = "macos"))]
            let mut icon = include_bytes!("../../icons/tray-icon.ico").to_vec();
            if *common_tray_icon {
                let icon_dir_path = dirs::app_home_dir()?.join("icons");
                let png_path = icon_dir_path.join("common.png");
                let ico_path = icon_dir_path.join("common.ico");
                if ico_path.exists() {
                    icon = std::fs::read(ico_path).unwrap();
                } else if png_path.exists() {
                    icon = std::fs::read(png_path).unwrap();
                }
            }
            icon
        };

        #[cfg(target_os = "macos")]
        {
            let is_template =
                crate::utils::help::is_monochrome_image_from_bytes(&icon_bytes).unwrap_or(false);
            let up_text = self.get_up_speed();
            let down_text = self.get_down_speed();
            let icon_bytes = Self::add_speed_text(icon_bytes, up_text, down_text)?;
            let _ = tray.set_icon(Some(tauri::image::Image::from_bytes(&icon_bytes)?));
            let _ = tray.set_icon_as_template(is_template);
        }

        #[cfg(not(target_os = "macos"))]
        let _ = tray.set_icon(Some(tauri::image::Image::from_bytes(&icon_bytes)?));

        Ok(())
    }

    /// 更新托盘提示
    pub fn update_tooltip(&self) -> Result<()> {
        let app_handle = handle::Handle::global().app_handle().unwrap();
        let use_zh = { Config::verge().latest().language == Some("zh".into()) };
        let version = VERSION.get().unwrap();

        let verge = Config::verge().latest().clone();
        let system_proxy = verge.enable_system_proxy.as_ref().unwrap_or(&false);
        let tun_mode = verge.enable_tun_mode.as_ref().unwrap_or(&false);

        let switch_map = {
            let mut map = std::collections::HashMap::new();
            map.insert(true, "on");
            map.insert(false, "off");
            map
        };

        let mut current_profile_name = "None".to_string();
        let profiles = Config::profiles();
        let profiles = profiles.latest();
        if let Some(current_profile_uid) = profiles.get_current() {
            let current_profile = profiles.get_item(&current_profile_uid);
            current_profile_name = match &current_profile.unwrap().name {
                Some(profile_name) => profile_name.to_string(),
                None => current_profile_name,
            };
        };

        let tray = app_handle.tray_by_id("main").unwrap();
        let _ = tray.set_tooltip(Some(&format!(
            "Clash Verge {version}\n{}: {}\n{}: {}\n{}: {}",
            t!("SysProxy", "系统代理", use_zh),
            switch_map[system_proxy],
            t!("TUN", "Tun模式", use_zh),
            switch_map[tun_mode],
            t!("Profile", "当前订阅", use_zh),
            current_profile_name
        )));
        Ok(())
    }

    pub fn update_part(&self) -> Result<()> {
        self.update_menu()?;
        self.update_icon()?;
        self.update_tooltip()?;
        Ok(())
    }

    fn get_up_speed(&self) -> String {
        let speed_rate = self.speed_rate.read();
        speed_rate
            .as_ref()
            .and_then(|rate| rate.up_text.read().as_ref().cloned())
            .unwrap_or_else(|| "0KB/s".to_string())
    }

    fn get_down_speed(&self) -> String {
        let speed_rate = self.speed_rate.read();
        speed_rate
            .as_ref()
            .and_then(|rate| rate.down_text.read().as_ref().cloned())
            .unwrap_or_else(|| "0KB/s".to_string())
    }

    /// 订阅流量数据
    pub async fn subscribe_traffic(&self) -> Result<()> {
        // 创建用于关闭的广播通道
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        // 保存发送端用于后续关闭
        *self.shutdown_tx.write() = Some(shutdown_tx);

        // 克隆需要的值
        let speed_rate: Arc<
            parking_lot::lock_api::RwLock<parking_lot::RawRwLock, Option<SpeedRate>>,
        > = self.speed_rate.clone();

        // 启动监听任务
        tauri::async_runtime::spawn(async move {
            let mut shutdown = shutdown_rx;

            if let Ok(mut stream) = get_traffic_stream().await {
                loop {
                    tokio::select! {
                        Some(traffic) = stream.next() => {
                            if let Ok(traffic) = traffic {
                                if let Some(rate) = speed_rate.read().as_ref() {
                                    rate.update_traffic(traffic.up, traffic.down);
                                }
                            }
                        }
                        _ = shutdown.recv() => break,
                    }
                }
            }
        });

        Ok(())
    }

    /// 取消订阅 traffic 数据
    pub fn unsubscribe_traffic(&self) {
        if let Some(tx) = self.shutdown_tx.write().take() {
            drop(tx); // 发送端被丢弃时会自动发送关闭信号
        }
    }
}

fn create_tray_menu(
    app_handle: &AppHandle,
    mode: Option<&str>,
    system_proxy_enabled: bool,
    tun_mode_enabled: bool,
) -> Result<tauri::menu::Menu<Wry>> {
    let mode = mode.unwrap_or("");
    let use_zh = { Config::verge().latest().language == Some("zh".into()) };
    let version = VERSION.get().unwrap();

    let open_window = &MenuItem::with_id(
        app_handle,
        "open_window",
        t!("Dashboard", "打开面板", use_zh),
        true,
        None::<&str>,
    )
    .unwrap();

    let rule_mode = &CheckMenuItem::with_id(
        app_handle,
        "rule_mode",
        t!("Rule Mode", "规则模式", use_zh),
        true,
        mode == "rule",
        None::<&str>,
    )
    .unwrap();

    let global_mode = &CheckMenuItem::with_id(
        app_handle,
        "global_mode",
        t!("Global Mode", "全局模式", use_zh),
        true,
        mode == "global",
        None::<&str>,
    )
    .unwrap();

    let direct_mode = &CheckMenuItem::with_id(
        app_handle,
        "direct_mode",
        t!("Direct Mode", "直连模式", use_zh),
        true,
        mode == "direct",
        None::<&str>,
    )
    .unwrap();

    let system_proxy = &CheckMenuItem::with_id(
        app_handle,
        "system_proxy",
        t!("System Proxy", "系统代理", use_zh),
        true,
        system_proxy_enabled,
        None::<&str>,
    )
    .unwrap();

    let tun_mode = &CheckMenuItem::with_id(
        app_handle,
        "tun_mode",
        t!("TUN Mode", "Tun模式", use_zh),
        true,
        tun_mode_enabled,
        None::<&str>,
    )
    .unwrap();

    let copy_env = &MenuItem::with_id(
        app_handle,
        "copy_env",
        t!("Copy Env", "复制环境变量", use_zh),
        true,
        None::<&str>,
    )
    .unwrap();

    let open_app_dir = &MenuItem::with_id(
        app_handle,
        "open_app_dir",
        t!("Conf Dir", "配置目录", use_zh),
        true,
        None::<&str>,
    )
    .unwrap();

    let open_core_dir = &MenuItem::with_id(
        app_handle,
        "open_core_dir",
        t!("Core Dir", "内核目录", use_zh),
        true,
        None::<&str>,
    )
    .unwrap();

    let open_logs_dir = &MenuItem::with_id(
        app_handle,
        "open_logs_dir",
        t!("Logs Dir", "日志目录", use_zh),
        true,
        None::<&str>,
    )
    .unwrap();
    let open_dir = &Submenu::with_id_and_items(
        app_handle,
        "open_dir",
        t!("Open Dir", "打开目录", use_zh),
        true,
        &[open_app_dir, open_core_dir, open_logs_dir],
    )
    .unwrap();

    let restart_clash = &MenuItem::with_id(
        app_handle,
        "restart_clash",
        t!("Restart Clash Core", "重启Clash内核", use_zh),
        true,
        None::<&str>,
    )
    .unwrap();

    let restart_app = &MenuItem::with_id(
        app_handle,
        "restart_app",
        t!("Restart App", "重启App", use_zh),
        true,
        None::<&str>,
    )
    .unwrap();

    let app_version = &MenuItem::with_id(
        app_handle,
        "app_version",
        format!("Version {version}"),
        true,
        None::<&str>,
    )
    .unwrap();

    let more = &Submenu::with_id_and_items(
        app_handle,
        "more",
        t!("More", "更多", use_zh),
        true,
        &[restart_clash, restart_app, app_version],
    )
    .unwrap();

    let quit = &MenuItem::with_id(
        app_handle,
        "quit",
        t!("Quit", "退出", use_zh),
        true,
        Some("CmdOrControl+Q"),
    )
    .unwrap();

    let separator = &PredefinedMenuItem::separator(app_handle).unwrap();

    let menu = tauri::menu::MenuBuilder::new(app_handle)
        .items(&[
            open_window,
            separator,
            rule_mode,
            global_mode,
            direct_mode,
            separator,
            system_proxy,
            tun_mode,
            copy_env,
            open_dir,
            more,
            separator,
            quit,
        ])
        .build()
        .unwrap();
    Ok(menu)
}

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

    /// 更新流量数据
    pub fn update_traffic(&self, up: u64, down: u64) {
        // 更新上传速率
        let mut up_text = self.up_text.write();
        *up_text = Some(format_bytes_speed(up));

        // 更新下载速率
        let mut down_text = self.down_text.write();
        *down_text = Some(format_bytes_speed(down));
    }
}

fn on_menu_event(_: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        mode @ ("rule_mode" | "global_mode" | "direct_mode") => {
            let mode = &mode[0..mode.len() - 5];
            println!("change mode to: {}", mode);
            feat::change_clash_mode(mode.into());
        }
        "open_window" => resolve::create_window(),
        "system_proxy" => feat::toggle_system_proxy(),
        "tun_mode" => feat::toggle_tun_mode(),
        "copy_env" => feat::copy_clash_env(),
        "open_app_dir" => crate::log_err!(cmds::open_app_dir()),
        "open_core_dir" => crate::log_err!(cmds::open_core_dir()),
        "open_logs_dir" => crate::log_err!(cmds::open_logs_dir()),
        "restart_clash" => feat::restart_clash_core(),
        "restart_app" => feat::restart_app(),
        "quit" => {
            println!("quit");
            feat::quit(Some(0));
        }
        _ => {}
    }
}

#[derive(Debug)]
pub struct Traffic {
    pub up: u64,
    pub down: u64,
}

async fn get_traffic_stream() -> Result<impl Stream<Item = Result<Traffic, anyhow::Error>>> {
    use futures::stream::{self, StreamExt};
    use std::time::Duration;

    let stream = Box::pin(
        stream::unfold((), |_| async {
            loop {
                // 获取配置
                let config_guard = Config::clash().latest().clone();
                let port = config_guard.get_mixed_port();
                let secret = config_guard.get_secret();

                // 构建 websocket URL
                let ws_url = if let Some(token) = secret {
                    format!("ws://127.0.0.1:{}/traffic?token={}", port, token)
                } else {
                    format!("ws://127.0.0.1:{}/traffic", port)
                };

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
