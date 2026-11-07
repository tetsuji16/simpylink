//! egui event → canvas event conversion tests (M1-08, issue #49).

use eframe::egui;
use sim_gui::canvas::controller::{CanvasEvent, CanvasKey, PointerButton};
use sim_gui::canvas::view::{Modifiers, ScreenPos, ScreenVec};
use sim_gui::canvas::widget::{convert_event, convert_key, convert_modifiers};

const ORIGIN: egui::Pos2 = egui::Pos2 { x: 10.0, y: 20.0 };

fn button(pos: egui::Pos2, b: egui::PointerButton, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: b,
        pressed,
        modifiers: egui::Modifiers::SHIFT,
    }
}

fn key(k: egui::Key, pressed: bool, mods: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: mods,
    }
}

#[test]
fn primary_down_is_offset_by_origin() {
    let ev = button(
        egui::Pos2::new(110.0, 70.0),
        egui::PointerButton::Primary,
        true,
    );
    assert_eq!(
        convert_event(&ev, ORIGIN),
        Some(CanvasEvent::PointerDown {
            pos: ScreenPos::new(100.0, 50.0),
            button: PointerButton::Primary,
            mods: Modifiers {
                shift: true,
                ..Default::default()
            },
        })
    );
}

#[test]
fn middle_up() {
    let ev = button(
        egui::Pos2::new(110.0, 70.0),
        egui::PointerButton::Middle,
        false,
    );
    assert_eq!(
        convert_event(&ev, ORIGIN),
        Some(CanvasEvent::PointerUp {
            pos: ScreenPos::new(100.0, 50.0),
            button: PointerButton::Middle,
            mods: Modifiers {
                shift: true,
                ..Default::default()
            },
        })
    );
}

#[test]
fn extra_button_is_ignored() {
    let ev = button(egui::Pos2::new(0.0, 0.0), egui::PointerButton::Extra1, true);
    assert_eq!(convert_event(&ev, ORIGIN), None);
}

#[test]
fn equals_key_maps_to_plus() {
    let ev = key(egui::Key::Equals, true, egui::Modifiers::COMMAND);
    assert_eq!(
        convert_event(&ev, ORIGIN),
        Some(CanvasEvent::Key {
            key: CanvasKey::Plus,
            pressed: true,
            mods: Modifiers {
                ctrl: true,
                ..Default::default()
            },
        })
    );
}

#[test]
fn unhandled_key_is_ignored() {
    assert_eq!(convert_key(egui::Key::F12), None);
    let ev = key(egui::Key::F12, true, egui::Modifiers::NONE);
    assert_eq!(convert_event(&ev, ORIGIN), None);
}

#[test]
fn copy_becomes_ctrl_c() {
    assert_eq!(
        convert_event(&egui::Event::Copy, ORIGIN),
        Some(CanvasEvent::Key {
            key: CanvasKey::C,
            pressed: true,
            mods: Modifiers {
                ctrl: true,
                ..Default::default()
            },
        })
    );
}

#[test]
fn wheel_line_scales_by_50() {
    let ev = egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::Vec2::new(0.0, 1.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    };
    assert_eq!(
        convert_event(&ev, ORIGIN),
        Some(CanvasEvent::Scroll {
            pos: ScreenPos::new(0.0, 0.0),
            delta: ScreenVec::new(0.0, 50.0),
            mods: Modifiers::default(),
        })
    );
}

#[test]
fn command_modifier_is_ctrl() {
    assert!(convert_modifiers(egui::Modifiers::COMMAND).ctrl);
    assert!(!convert_modifiers(egui::Modifiers::NONE).ctrl);
}

#[test]
fn pointer_moved_and_gone() {
    assert_eq!(
        convert_event(
            &egui::Event::PointerMoved(egui::Pos2::new(15.0, 25.0)),
            ORIGIN
        ),
        Some(CanvasEvent::PointerMove {
            pos: ScreenPos::new(5.0, 5.0),
            mods: Modifiers::default(),
        })
    );
    assert_eq!(
        convert_event(&egui::Event::PointerGone, ORIGIN),
        Some(CanvasEvent::PointerLeft)
    );
}

#[test]
fn cut_and_paste_become_ctrl_shortcuts() {
    assert_eq!(
        convert_event(&egui::Event::Cut, ORIGIN),
        Some(CanvasEvent::Key {
            key: CanvasKey::X,
            pressed: true,
            mods: Modifiers {
                ctrl: true,
                ..Default::default()
            },
        })
    );
    assert_eq!(
        convert_event(&egui::Event::Paste("x".into()), ORIGIN),
        Some(CanvasEvent::Key {
            key: CanvasKey::V,
            pressed: true,
            mods: Modifiers {
                ctrl: true,
                ..Default::default()
            },
        })
    );
}
