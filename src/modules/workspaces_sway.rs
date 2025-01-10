use std::{
    any::TypeId,
    sync::{Arc, RwLock},
};

use crate::{
    config::AppearanceColor,
    style::{header_pills, WorkspaceButtonStyle},
};
use log::{debug, error};

use futures_lite::StreamExt;
use iced::stream::channel;
use iced::{
    alignment,
    widget::{container, text},
    Subscription,
};
use iced::{widget::button, Length};
use iced::{widget::Row, Element};
use swayipc_async::Event;
use swayipc_async::EventType;

#[derive(Debug, Clone)]
pub struct WorkspaceSway {
    pub id: i64,
    pub name: String,
    pub output: String,
    pub active: bool,
    pub urgent: bool,
}

fn get_workspaces() -> Vec<WorkspaceSway> {
    swayipc::Connection::new().map_or_else(
        |_| {
            error!("Failled to connect to sway IPC");
            vec![]
        },
        |mut connection| {
            let mut workspaces = connection
                .get_workspaces()
                .unwrap_or_default()
                .iter()
                .map(|workspace| WorkspaceSway {
                    id: workspace.id,
                    name: workspace.name.clone(),
                    output: workspace.output.clone(),
                    active: workspace.focused,
                    urgent: workspace.urgent,
                })
                .collect::<Vec<_>>();
            workspaces.sort_by(|a, b| a.name.cmp(&b.name));
            workspaces
        },
    )
}

pub struct WorkspacesSway {
    workspaces: Vec<WorkspaceSway>,
}

#[derive(Debug, Clone)]
pub enum Message {
    WorkspacesChanged(Vec<WorkspaceSway>),
    ChangeWorkspace(String),
}

impl Default for WorkspacesSway {
    fn default() -> Self {
        Self {
            workspaces: get_workspaces(),
        }
    }
}

impl WorkspacesSway {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::WorkspacesChanged(workspaces) => {
                self.workspaces = workspaces;
            }
            Message::ChangeWorkspace(name) => {
                let already_active = self
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.active && workspace.name == name);

                if !already_active {
                    debug!("changing workspace to: {}", name);
                    if let Err(e) = swayipc::Connection::new()
                        .unwrap()
                        .run_command(format!("workspace number {}", name))
                    {
                        error!("failed to dispatch workspace change: {:?}", e);
                    }
                }
            }
        }
    }

    pub fn view(&self, workspace_colors: &[AppearanceColor]) -> Element<Message> {
        container(
            Row::with_children(
                self.workspaces
                    .iter()
                    .map(|workspace| {
                        let color = workspace_colors.get(0).copied();
                        button(
                            container(text(workspace.name.as_str()).size(10))
                                .align_x(alignment::Horizontal::Center)
                                .align_y(alignment::Vertical::Center),
                        )
                        .style(WorkspaceButtonStyle(false, Some(color)).into_style())
                        .padding(if workspace.active { [0, 16] } else { [0, 8] })
                        .on_press(Message::ChangeWorkspace(workspace.name.clone()))
                        // .width(if workspace.active {
                        //     Length::Fixed(32.)
                        // } else {
                        //     Length::Fixed(16.)
                        // })
                        .height(16)
                        .into()
                    })
                    .collect::<Vec<Element<'_, _, _>>>(),
            )
            .spacing(4),
        )
        .padding([4, 8])
        .align_y(alignment::Vertical::Center)
        .height(Length::Shrink)
        .style(header_pills)
        .into()
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
                    .subscribe([EventType::Workspace, EventType::Output])
                    .await
                    .unwrap();
                while let Some(event) = events.next().await.transpose().unwrap() {
                    match event {
                        Event::Workspace(_) => {
                            if let Ok(mut output) = output.write() {
                                output
                                    .try_send(Message::WorkspacesChanged(get_workspaces()))
                                    .expect("Error");
                            }
                        }
                        Event::Output(_) => {
                            if let Ok(mut output) = output.write() {
                                output
                                    .try_send(Message::WorkspacesChanged(get_workspaces()))
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
