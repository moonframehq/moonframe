//! Writes what the window hits for `--check`, and the selection's report
//! when it changed, every [`TICK`], until the window closes (see
//! `crate::live`).

use {super::*, crate::live::TICK};

pub(super) fn ticking(cx: &mut Context<MoonframeApp>) {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(TICK).await;
            if this
                .update(cx, |this, cx| {
                    this.live.tick(&this.workspace);
                    if this.refresh_selection() {
                        cx.notify();
                    }
                })
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
}
