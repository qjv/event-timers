use nexus::imgui::Ui;
use crate::config::TimeRulerInterval;
use crate::time_utils::{calculate_tyria_time, format_time_only};

/// Render the time ruler
/// - `label_offset`: horizontal offset for the timeline portion (when labels are on the left)
/// - `tick_interval`: interval between tick marks
/// - `show_current_time`: whether to display the current time text on the ruler
pub fn render_time_ruler(
    ui: &Ui,
    current_time: i64,
    view_range: f32,
    time_position: f32,
    label_offset: f32,
    tick_interval: TimeRulerInterval,
    show_current_time: bool,
) {
    let draw_list = ui.get_window_draw_list();
    let cursor_pos = ui.cursor_screen_pos();
    let available_width = ui.content_region_avail()[0];
    let ruler_height = 20.0;

    // Timeline starts after label offset
    let timeline_start_x = cursor_pos[0] + label_offset;
    let timeline_width = available_width - label_offset;

    // Draw background for entire ruler (including label area)
    draw_list.add_rect(
        cursor_pos,
        [cursor_pos[0] + available_width, cursor_pos[1] + ruler_height],
        [0.15, 0.15, 0.15, 1.0],
    )
    .filled(true)
    .build();

    // Tick at configured interval
    let tick_interval_seconds = tick_interval.as_seconds();
    let time_before_current = view_range * time_position;
    let time_after_current = view_range * (1.0 - time_position);
    let pixels_per_second = timeline_width / view_range;

    let start_time = current_time - time_before_current as i64;
    let first_tick = ((start_time / tick_interval_seconds) + 1) * tick_interval_seconds;

    // Calculate max iterations needed
    let max_ticks = ((time_before_current + time_after_current) / tick_interval_seconds as f32).ceil() as i64 + 1;

    for i in 0..max_ticks {
        let tick_time = first_tick + (i * tick_interval_seconds);
        let offset_from_current = tick_time - current_time;

        if offset_from_current >= -time_before_current as i64 && offset_from_current <= time_after_current as i64 {
            let x_pos = timeline_start_x + ((offset_from_current as f32 + time_before_current) * pixels_per_second);

            draw_list.add_line(
                [x_pos, cursor_pos[1] + ruler_height - 8.0],
                [x_pos, cursor_pos[1] + ruler_height],
                [0.6, 0.6, 0.6, 1.0],
            )
            .thickness(1.0)
            .build();
        }
    }

    // Current time red line - positioned within timeline area
    let current_time_x = timeline_start_x + (time_position * timeline_width);
    draw_list.add_line(
        [current_time_x, cursor_pos[1]],
        [current_time_x, cursor_pos[1] + ruler_height],
        [1.0, 0.0, 0.0, 1.0],
    )
    .thickness(2.0)
    .build();

    // Display current time text on the ruler if enabled
    if show_current_time {
        let time_text = format_time_only(current_time);
        let text_size = ui.calc_text_size(&time_text);

        // Position the text to the left of the current time line, or right if not enough space
        let text_x = if current_time_x - text_size[0] - 5.0 >= timeline_start_x {
            current_time_x - text_size[0] - 5.0
        } else {
            current_time_x + 5.0
        };
        let text_y = cursor_pos[1] + (ruler_height - text_size[1]) / 2.0;

        draw_list.add_text([text_x, text_y], [1.0, 1.0, 1.0, 0.9], &time_text);
    }

    ui.dummy([available_width, ruler_height]);

    if ui.is_item_hovered() {
        let mouse_pos = ui.io().mouse_pos;
        let mouse_x = mouse_pos[0] - timeline_start_x;

        // Only show tooltip if mouse is over the timeline portion
        if mouse_x >= 0.0 && mouse_x <= timeline_width {
            let time_offset = (mouse_x * view_range / timeline_width) - time_before_current;
            let hover_time = current_time + time_offset as i64;

            let tyria_time = calculate_tyria_time(hover_time);

            ui.tooltip(|| {
                ui.text(format!("Local: {}", format_time_only(hover_time)));
                ui.text(format!("Tyria: {:02}:{:02}", tyria_time.0, tyria_time.1));
            });
        }
    }
}