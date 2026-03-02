use std::sync::{Arc, Mutex};

use napi::bindgen_prelude::*;
use takumi::{
  layout::{Viewport, node::NodeKind},
  rendering::{
    AnimationFrame, RenderOptionsBuilder, encode_animated_png, encode_animated_webp, render,
  },
};

use crate::{
  ExternalMemoryAccountable, map_error,
  renderer::{AnimationOutputFormat, RendererState},
};

pub struct RenderAnimationTask {
  pub nodes: Option<Vec<(NodeKind, u32)>>,
  pub(crate) state: Arc<Mutex<RendererState>>,
  pub viewport: Viewport,
  pub format: AnimationOutputFormat,
  pub draw_debug_border: bool,
}

impl Task for RenderAnimationTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    let Some(nodes) = self.nodes.take() else {
      unreachable!()
    };
    let state = self
      .state
      .lock()
      .map_err(|e| Error::from_reason(format!("Renderer lock poisoned: {e}")))?;

    let frames = nodes
      .into_iter()
      .map(|(node, duration_ms)| {
        Ok(AnimationFrame::new(
          render(
            RenderOptionsBuilder::default()
              .viewport(self.viewport)
              .node(node)
              .global(&state.global)
              .draw_debug_border(self.draw_debug_border)
              .build()
              .map_err(map_error)?,
          )
          .map_err(map_error)?,
          duration_ms,
        ))
      })
      .collect::<Result<Vec<_>, _>>()?;

    let mut buffer = Vec::new();

    match self.format {
      AnimationOutputFormat::webp => {
        encode_animated_webp(&frames, &mut buffer, true, false, None)
          .map_err(|e| napi::Error::from_reason(e.to_string()))?;
      }
      AnimationOutputFormat::apng => {
        encode_animated_png(&frames, &mut buffer, None)
          .map_err(|e| napi::Error::from_reason(e.to_string()))?;
      }
    }

    Ok(buffer)
  }

  fn resolve(&mut self, mut env: Env, output: Self::Output) -> Result<Self::JsValue> {
    // Account external memory to V8's garbage collector
    // This enables V8 to collect memory based on actual memory pressure
    output.account_external_memory(&mut env)?;
    Ok(output.into())
  }
}
