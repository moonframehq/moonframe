//! `ui.map`, without reaching a tile server: what its mod hears.

use super::*;

#[test]
fn a_pin_sends_its_own_message() {
    let main = r#"
return mod {
	init = function(previous) return previous or { said = "nothing" } end,
	update = function(state, msg)
		if type(msg) == "table" and msg.stop then state.said = "stop " .. msg.stop end
	end,
	view = function(state)
		return ui.column { grow = true,
			ui.text { "said:" .. state.said },
			ui.map { center = { 0, 0 }, zoom = 3, height = 300,
				pins = { { lat = 0, lon = 0, msg = { stop = 7 } } } },
		}
	end,
}
"#;
    let dir = folder(main);
    let mut ui = Ui::open(dir.path().to_owned(), None);
    ui.waker = Some(Arc::new(|| {}));
    let mut window = Runtime::new(ui, view, (800., 400.), Env::new(true, 2.));
    settle(&mut window);
    let map = window
        .elements()
        .iter()
        .filter(|element| element.rect[3] >= 299.)
        .map(|element| element.rect)
        .next_back()
        .expect("the map");
    let middle = [map[0] + map[2] / 2. - 1., map[1] + map[3] / 2. - 1., 2., 2.];
    click_at(&mut window, middle);
    assert!(shows(&window, "said:stop 7"));
}

#[test]
fn a_map_tells_its_mod_of_a_wheel_zoom_as_of_a_drag() {
    let main = r#"
return mod {
	init = function(previous) return previous or { zoom = 0 } end,
	update = function(state, msg)
		if type(msg) == "table" and msg.moved then state.zoom = msg.zoom end
	end,
	view = function(state)
		return ui.column { grow = true,
			ui.text { "zoom:" .. math.round(state.zoom) },
			ui.map { center = { 0, 0 }, zoom = 3, height = 300, move = { moved = true } },
		}
	end,
}
"#;
    let dir = folder(main);
    let mut ui = Ui::open(dir.path().to_owned(), None);
    ui.waker = Some(Arc::new(|| {}));
    let mut window = Runtime::new(ui, view, (800., 400.), Env::new(true, 2.));
    settle(&mut window);
    window.update(|ui, tx| {
        let start = map::Camera {
            center: [0., 0.],
            zoom: 3.,
        };
        ui.map_tiles
            .cameras
            .borrow_mut()
            .insert("m".to_owned(), start);
        ui.map_tiles
            .cameras
            .borrow_mut()
            .entry("m".to_owned())
            .and_modify(|camera| camera.zoom = 5.);
        let hearer = map::Hearer {
            owner: "shell".to_owned(),
            to: "shell".to_owned(),
            msg: Data::map([("moved", true.into())]),
        };
        ui.map_moved_by_user("m", hearer, false, tx);
    });
    settle(&mut window);
    assert!(shows(&window, "zoom:5"));
}
