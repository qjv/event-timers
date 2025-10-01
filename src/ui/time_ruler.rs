use nexus::imgui::Ui;
use crate::time_utils::{calculate_tyria_time, format_time_only};

pub fn render_time_ruler(ui: &Ui, current_time: i64, view_range: f32, time_position: f32) {
    let draw_list = ui.get_window_draw_list();
    let cursor_pos = ui.cursor_screen_pos();
    let available_width = ui.content_region_avail()[0];
    let ruler_height = 20.0;
    
    draw_list.add_rect(
        cursor_pos,
        [cursor_pos[0] + available_width, cursor_pos[1] + ruler_height],
        [0.15, 0.15, 0.15, 1.0],
    )
    .filled(true)
    .build();
    
    // Tick every 5 real minutes
    const TICK_INTERVAL: i64 = 300;
    let time_before_current = view_range * time_position;
    let time_after_current = view_range * (1.0 - time_position);
    let pixels_per_second = available_width / view_range;
    
    let start_time = current_time - time_before_current as i64;
    let first_tick = ((start_time / TICK_INTERVAL) + 1) * TICK_INTERVAL;
    
    // Calculate max iterations needed instead of fixed 50
    let max_ticks = ((time_before_current + time_after_current) / TICK_INTERVAL as f32).ceil() as i64 + 1;
    
    for i in 0..max_ticks {
        let tick_time = first_tick + (i * TICK_INTERVAL);
        let offset_from_current = tick_time - current_time;
        
        if offset_from_current >= -time_before_current as i64 && offset_from_current <= time_after_current as i64 {
            let x_pos = cursor_pos[0] + ((offset_from_current as f32 + time_before_current) * pixels_per_second);
            
            draw_list.add_line(
                [x_pos, cursor_pos[1] + ruler_height - 8.0],
                [x_pos, cursor_pos[1] + ruler_height],
                [0.6, 0.6, 0.6, 1.0],
            )
            .thickness(1.0)
            .build();
        }
    }
    
    let current_time_x = cursor_pos[0] + (time_position * available_width);
    draw_list.add_line(
        [current_time_x, cursor_pos[1]],
        [current_time_x, cursor_pos[1] + ruler_height],
        [1.0, 0.0, 0.0, 1.0],
    )
    .thickness(2.0)
    .build();
    
    ui.dummy([available_width, ruler_height]);
    
    if ui.is_item_hovered() {
        let mouse_pos = ui.io().mouse_pos;
        let mouse_x = mouse_pos[0] - cursor_pos[0];
        let time_offset = (mouse_x * view_range / available_width) - time_before_current;
        let hover_time = current_time + time_offset as i64;
        
        let tyria_time = calculate_tyria_time(hover_time);
        
        ui.tooltip(|| {
            ui.text(format!("Local: {}", format_time_only(hover_time)));
            ui.text(format!("Tyria: {:02}:{:02}", tyria_time.0, tyria_time.1));
        });
    }
}