use yew::prelude::*;
use std::process::Command;
use std::fs::{self, DirEntry};

#[function_component]
fn App() -> Html {
    let counter = use_state(|| 0);
    let path = use_state(|| ".");
    let onclick = {
        let counter = counter.clone();
        let path = path.clone();
        move |_| {
            let value = *counter + 1;
            counter.set(value);

            // Build docker image
            let _build_result = Command::new("oct-cli")
                .arg("deploy")
                .arg("--platform")
                .arg("linux/amd64")
                .arg("--dockerfile-path")
                .arg("../../examples/decentraland_stream/")
                .arg("context-path")
                .arg("../../examples/decentraland_stream/")
                .output();

            println!("Docker image {} built successfully", "yO");

            }
    };

    html! {
        <div>
            <button {onclick}>{ "deploy" }</button>
            <p>{ *counter }</p>
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
