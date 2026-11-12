//! Writes what the window hits for `--check`, every [`TICK`], until the
//! window closes (see `crate::live`).

use {super::*, crate::live::TICK};

pub(super) fn ticking(cx: &mut Context<MoonframeApp>) {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(TICK).await;
            if this
                .update(cx, |this, _| this.live.tick(&this.workspace))
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
}
