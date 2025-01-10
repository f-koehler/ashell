use crate::style::header_pills;
use futures_lite::StreamExt;
use iced::{
    stream::channel,
    widget::{container, text},
    Element, Subscription,
};
use log::error;
use std::{
    any::TypeId,
    sync::{Arc, RwLock},
};
use swayipc::WindowChange;
use swayipc_async::{Event, EventType};

fn get_active_window_title() -> Option<String> {
    swayipc::Connection::new().map_or_else(
        |_| {
            error!("Failed to connect to sway IPC");
            None
        },
        |mut connection| {
            connection
                .get_tree()
                .unwrap()
                .iter()
                .find(|node| node.focused)
                .map_or(None, |node| node.name.clone())
        },
    )
}

pub struct TitleSway {
    value: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TitleChanged(Option<String>),
}

impl Default for TitleSway {
    fn default() -> Self {
        Self {
            value: get_active_window_title(),
        }
    }
}

impl TitleSway {
    pub fn update(&mut self, message: Message, truncate_title_after_length: u32) {
        match message {
            Message::TitleChanged(value) => {
                if let Some(value) = value {
                    let length = value.len();

                    self.value = Some(if length > truncate_title_after_length as usize {
                        let split = truncate_title_after_length as usize / 2;
                        let first_part = value.chars().take(split).collect::<String>();
                        let last_part = value.chars().skip(length - split).collect::<String>();
                        format!("{}...{}", first_part, last_part)
                    } else {
                        value
                    });
                } else {
                    self.value = None;
                }
            }
        }
    }

    pub fn view(&self) -> Option<Element<Message>> {
        self.value.as_ref().map(|value| {
            container(text(value).size(12))
                .padding([2, 7])
                .style(header_pills)
                .into()
        })
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let id = TypeId::of::<Self>();
        Subscription::run_with_id(
            id,
            channel(10, |output| async move {
                let output = Arc::new(RwLock::new(output));
                let mut events = swayipc_async::Connection::new()
                    .await
                    .unwrap()
                    .subscribe([EventType::Window, EventType::Workspace])
                    .await
                    .unwrap();
                while let Some(event) = events.next().await.transpose().unwrap() {
                    match event {
                        Event::Window(event) => match event.as_ref().change {
                            WindowChange::Focus => {
                                if let Ok(mut output) = output.write() {
                                    output
                                        .try_send(Message::TitleChanged(event.container.name))
                                        .expect("Error");
                                }
                            }
                            WindowChange::Title => {
                                if event.container.focused {
                                    if let Ok(mut output) = output.write() {
                                        output
                                            .try_send(Message::TitleChanged(event.container.name))
                                            .expect("Error");
                                    }
                                }
                            }
                            _ => {}
                        },
                        Event::Workspace(_) => {
                            if let Ok(mut output) = output.write() {
                                output
                                    .try_send(Message::TitleChanged(get_active_window_title()))
                                    .expect("Error");
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }),
        )
    }
}
