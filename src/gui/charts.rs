use eframe::egui::{self, Color32, Pos2, Stroke};

/// Coordinates are approximate display values only. No chart data feeds the engine.
pub fn chart(ui: &mut egui::Ui, title: &str, unit: &str, series: &[(&str, Vec<(i32, f64)>)]) {
    ui.strong(title);
    let points: Vec<_> = series.iter().flat_map(|(_, p)| p.iter()).collect();
    if points.is_empty() {
        ui.weak("Advance the simulation to build history.");
        return;
    }
    let xmin = points.iter().map(|p| p.0).min().unwrap_or(0);
    let xmax = points.iter().map(|p| p.0).max().unwrap_or(xmin);
    let ymin = points.iter().map(|p| p.1).fold(0.0_f64, f64::min);
    let ymax = points.iter().map(|p| p.1).fold(0.0_f64, f64::max);
    ui.small(format!(
        "{unit} • {ymin:.2} … {ymax:.2} • years {xmin}–{xmax}"
    ));
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 150.0),
        egui::Sense::hover(),
    );
    let plot = rect.shrink(8.0);
    let map = |x: i32, y: f64| {
        Pos2::new(
            plot.left()
                + ((f64::from(x) - f64::from(xmin)) / (f64::from(xmax) - f64::from(xmin)).max(1.0))
                    as f32
                    * plot.width(),
            plot.bottom() - ((y - ymin) / (ymax - ymin).max(1.0)) as f32 * plot.height(),
        )
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 5.0, Color32::from_rgb(17, 24, 35));
    painter.line_segment(
        [map(xmin, 0.0), map(xmax, 0.0)],
        Stroke::new(1.0, Color32::GRAY),
    );
    let colors = [
        Color32::from_rgb(92, 206, 201),
        Color32::from_rgb(246, 186, 97),
    ];
    for (index, (_, data)) in series.iter().enumerate() {
        let color = colors[index % colors.len()];
        for pair in data.windows(2) {
            painter.line_segment(
                [map(pair[0].0, pair[0].1), map(pair[1].0, pair[1].1)],
                Stroke::new(2.0, color),
            );
        }
        for &(x, y) in data {
            painter.circle_filled(map(x, y), if index == 0 { 2.5 } else { 1.5 }, color);
        }
    }
    if let Some(pos) = response.hover_pos() {
        let year = (f64::from(xmin)
            + f64::from(((pos.x - plot.left()) / plot.width()).clamp(0.0, 1.0))
                * (f64::from(xmax) - f64::from(xmin)))
        .round() as i32;
        response.on_hover_ui(|ui| {
            for (name, data) in series {
                if let Some((year, value)) = data
                    .iter()
                    .min_by_key(|p| (i64::from(p.0) - i64::from(year)).abs())
                {
                    ui.label(format!("{name} • {year}: {value:.2} {unit}"));
                }
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        for (i, (name, _)) in series.iter().enumerate() {
            ui.colored_label(
                colors[i % colors.len()],
                format!("{} {name}", if i == 0 { "●" } else { "·" }),
            );
        }
    });
    ui.add_space(12.0);
}
