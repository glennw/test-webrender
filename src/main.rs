#![feature(box_syntax)]
#![feature(slice_patterns)]
#![feature(step_by)]

extern crate webrender;
extern crate glutin;
extern crate gleam;
extern crate png;
extern crate stb_image;
extern crate string_cache;
extern crate euclid;

use stb_image::image as stb_image2;

use gleam::gl;

use std::fs::File;
use std::io::Read;
use std::mem;
use std::path::PathBuf;
use std::ffi::CStr;

use webrender::{StackingContext, DisplayListBuilder, PipelineId};
use webrender::{StackingLevel, Epoch, GradientStop, ClipRegion};
use webrender::{Au, ColorF, GlyphInstance, ImageFormat, BorderSide, BorderRadius, BorderStyle};
use webrender::renderer;
use string_cache::Atom;
use euclid::{Point2D, Size2D, Rect, Matrix4};

struct ImageResource {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
}

fn load_file(name: &str) -> Vec<u8> {
    let mut file = File::open(name).unwrap();
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).unwrap();
    buffer
}

// TODO(pcwalton): Speed up with SIMD, or better yet, find some way to not do this.
pub fn byte_swap(data: &mut [u8]) {
    let length = data.len();
    for i in (0..length).step_by(4) {
        let r = data[i + 2];
        data[i + 2] = data[i + 0];
        data[i + 0] = r;
    }
}

// TODO(pcwalton): Speed up with SIMD, or better yet, find some way to not do this.
fn byte_swap_and_premultiply(data: &mut [u8]) {
    let length = data.len();
    for i in (0..length).step_by(4) {
        let r = data[i + 2];
        let g = data[i + 1];
        let b = data[i + 0];
        let a = data[i + 3];
        data[i + 0] = ((r as u32) * (a as u32) / 255) as u8;
        data[i + 1] = ((g as u32) * (a as u32) / 255) as u8;
        data[i + 2] = ((b as u32) * (a as u32) / 255) as u8;
    }
}

fn is_gif(buffer: &[u8]) -> bool {
    match buffer {
        [b'G',b'I',b'F',b'8', n, b'a', ..] if n == b'7' || n == b'9' => true,
        _ => false
    }
}

fn load_from_memory(buffer: &[u8]) -> Option<ImageResource> {
    if buffer.len() == 0 {
        return None;
    }

    if png::is_png(buffer) {
        match png::load_png_from_memory(buffer) {
            Ok(mut png_image) => {
                let (bytes, format) = match png_image.pixels {
                    png::PixelsByColorType::K8(ref mut _data) => {
                        panic!("todo");
                        //(data, ImageFormat::A8)
                    }
                    png::PixelsByColorType::KA8(ref mut _data) => {
                        panic!("todo");
                        //(data, PixelFormat::KA8)
                    }
                    png::PixelsByColorType::RGB8(ref mut data) => {
                        byte_swap(data);
                        (data, ImageFormat::RGB8)
                    }
                    png::PixelsByColorType::RGBA8(ref mut data) => {
                        byte_swap_and_premultiply(data);
                        (data, ImageFormat::RGBA8)
                    }
                };

                let bytes = mem::replace(bytes, Vec::new());

                let image = ImageResource {
                    width: png_image.width,
                    height: png_image.height,
                    format: format,
                    bytes: bytes,
                };

                Some(image)
            }
            Err(_err) => None,
        }
    } else {
        // For non-png images, we use stb_image
        // Can't remember why we do this. Maybe it's what cairo wants
        static FORCE_DEPTH: usize = 4;

        match stb_image2::load_from_memory_with_depth(buffer, FORCE_DEPTH, true) {
            stb_image2::LoadResult::ImageU8(mut image) => {
                assert!(image.depth == 4);
                // handle gif separately because the alpha-channel has to be premultiplied
                if is_gif(buffer) {
                    byte_swap_and_premultiply(&mut image.data);
                } else {
                    byte_swap(&mut image.data);
                }
                Some(ImageResource {
                    width: image.width as u32,
                    height: image.height as u32,
                    format: ImageFormat::RGBA8,
                    bytes: image.data,
                })
            }
            stb_image2::LoadResult::ImageF32(_image) => {
                panic!("HDR images not implemented");
                //None
            }
            stb_image2::LoadResult::Error(e) => {
                panic!("stb_image failed: {}", e);
                //None
            }
        }
    }
}

struct Notifier {
    window_proxy: glutin::WindowProxy,
}

impl Notifier {
    fn new(window_proxy: glutin::WindowProxy) -> Notifier {
        Notifier {
            window_proxy: window_proxy,
        }
    }
}

impl webrender::RenderNotifier for Notifier {
    fn new_frame_ready(&mut self) {
        //println!("wakeup!!");
        self.window_proxy.wakeup_event_loop();
    }
}

fn test1(api: &webrender::RenderApi,
         width: u32,
         height: u32) -> webrender::StackingContext {
    let font_id = Atom::from_slice("/usr/share/fonts/truetype/ttf-dejavu/DejaVuSerif-Italic.ttf");
    let font_bytes = load_file(font_id.as_slice());
    api.add_font(font_id.clone(), font_bytes);

    let clip_rect = Rect::new(Point2D::new(0.0, 0.0), Size2D::new(width as f32, height as f32));

    let color_red = ColorF::new(1.0, 0.0, 0.0, 1.0);
    let color_green = ColorF::new(0.0, 1.0, 0.0, 1.0);
    //let color_blue = ColorF::new(0.0, 0.0, 1.0, 1.0);
    let color_yellow = ColorF::new(1.0, 1.0, 0.0, 1.0);

    let clip = ClipRegion::new(Rect::new(Point2D::new(0.0, 0.0), Size2D::new(2048.0, 2048.0)));

    let mut dl = DisplayListBuilder::new();

    let rect = Rect::new(Point2D::new(100.0, 100.0), Size2D::new(100.0, 100.0));
    dl.push_rect(StackingLevel::Content, rect, clip.clone(), color_green);

    let glyphs = vec![
        GlyphInstance {
            index: 48,
            x: 100.0,
            y: 100.0,
        },
        GlyphInstance {
            index: 68,
            x: 150.0,
            y: 100.0,
        },
        GlyphInstance {
            index: 80,
            x: 200.0,
            y: 100.0,
        },
        GlyphInstance {
            index: 82,
            x: 250.0,
            y: 100.0,
        },
        GlyphInstance {
            index: 81,
            x: 300.0,
            y: 100.0,
        },
        GlyphInstance {
            index: 3,
            x: 350.0,
            y: 100.0,
        },
        GlyphInstance {
            index: 86,
            x: 400.0,
            y: 100.0,
        },
        GlyphInstance {
            index: 79,
            x: 450.0,
            y: 100.0,
        },
        GlyphInstance {
            index: 72,
            x: 500.0,
            y: 100.0,
        },
        GlyphInstance {
            index: 83,
            x: 550.0,
            y: 100.0,
        },
        GlyphInstance {
            index: 87,
            x: 600.0,
            y: 100.0,
        },
        GlyphInstance {
            index: 17,
            x: 650.0,
            y: 100.0,
        },
    ];

    dl.push_text(StackingLevel::Content, clip_rect, clip.clone(), glyphs, font_id, color_red, Au::from_px(50));

    let stretch_size = Size2D::new(100.0, 100.0);

    //let image_path = Atom::from_slice("/home/gw/rust-0.png");
    let image_path = "/home/gw/code/work/servo_gfx/servo/tests/html/doge-servo.jpg";
    let image_bytes = load_file(image_path);
    let image = load_from_memory(&image_bytes).unwrap();
    let image_id = api.add_image(image.width, image.height, image.format, image.bytes);

    let image_rect = Rect::new(Point2D::new(600.0, 100.0), Size2D::new(100.0, 100.0));
    dl.push_image(StackingLevel::Content, image_rect, clip.clone(), stretch_size.clone(), image_id);

    let image_rect2 = Rect::new(Point2D::new(600.0, 400.0), Size2D::new(100.0, 100.0));
    dl.push_image(StackingLevel::Content, image_rect2, clip.clone(), stretch_size, image_id);

    let grect = Rect::new(Point2D::new(0.0, 0.0), Size2D::new(100.0, 100.0));
    let start_point = Point2D::new(0.0, 0.0);
    let end_point = Point2D::new(100.0, 100.0);
    let stops = vec![
        GradientStop { offset: 0.0, color: ColorF::new(1.0, 0.0, 0.0, 1.0) },
        GradientStop { offset: 0.5, color: ColorF::new(0.0, 0.0, 1.0, 1.0) },
        GradientStop { offset: 1.0, color: ColorF::new(0.0, 1.0, 0.0, 1.0) },
    ];
    dl.push_gradient(StackingLevel::Content, grect, clip.clone(), start_point, end_point, stops);

    let left_border = BorderSide {
        width: 200.0,
        color: ColorF::new(1.0, 0.0, 0.0, 1.0),
        style: BorderStyle::Solid,
    };
    let top_border = BorderSide {
        width: 200.0,
        color: ColorF::new(0.0, 1.0, 0.0, 1.0),
        style: BorderStyle::Solid,
    };
    let right_border = BorderSide {
        width: 20.0,
        color: ColorF::new(0.0, 0.0, 1.0, 1.0),
        style: BorderStyle::Solid,
    };
    let bottom_border = BorderSide {
        width: 20.0,
        color: ColorF::new(1.0, 1.0, 0.0, 1.0),
        style: BorderStyle::Solid,
    };
    let radius = BorderRadius {
        top_left: Size2D::new(200.0, 200.0),
        top_right: Size2D::new(0.0, 0.0),
        bottom_left: Size2D::new(0.0, 0.0),
        bottom_right: Size2D::new(0.0, 0.0),
    };
    let border_rect = Rect::new(Point2D::new(100.0, 100.0), Size2D::new(400.0, 400.0));
    dl.push_border(StackingLevel::Content, border_rect, clip.clone(), left_border, top_border, right_border, bottom_border, radius);

    let overlap_rect_1 = Rect::new(Point2D::new(200.0, 500.0), Size2D::new(100.0, 100.0));
    let overlap_rect_2 = Rect::new(Point2D::new(250.0, 500.0), Size2D::new(100.0, 100.0));
    dl.push_rect(StackingLevel::Content, overlap_rect_1, clip.clone(), ColorF::new(1.0, 0.0, 0.0, 0.5));
    dl.push_rect(StackingLevel::Content, overlap_rect_2, clip.clone(), ColorF::new(0.0, 1.0, 0.0, 1.0));

    let display_list_id = api.add_display_list(dl, PipelineId(0), Epoch(0));

    let mut sc = StackingContext::new(Some(webrender::ScrollLayerId::new(0)),
                                      Rect::new(Point2D::new(0.0, 0.0), Size2D::new(width as f32, height as f32)),
                                      Rect::new(Point2D::new(0.0, 0.0), Size2D::new(width as f32, height as f32)),
                                      0,
                                      &Matrix4::identity(),
                                      &Matrix4::identity(),
                                      false,
                                      webrender::MixBlendMode::Normal);
    sc.add_display_list(display_list_id);

    let mut dl2 = DisplayListBuilder::new();
    let rect2 = Rect::new(Point2D::new(0.0, 0.0), Size2D::new(100.0, 100.0));
    dl2.push_rect(StackingLevel::Content, rect2, clip.clone(), color_yellow);
    let mut sc2 = StackingContext::new(None,
                                       Rect::new(Point2D::new(100.0, 600.0), Size2D::new(100.0 as f32, 100.0 as f32)),
                                       Rect::new(Point2D::new(0.0, 0.0), Size2D::new(100.0 as f32, 100.0 as f32)),
                                       0,
                                       &Matrix4::identity(),
                                       &Matrix4::identity(),
                                       false,
                                       webrender::MixBlendMode::Normal);
    let dl2_id = api.add_display_list(dl2, PipelineId(0), Epoch(0));
    sc2.add_display_list(dl2_id);
    sc.add_stacking_context(sc2);

    sc
}

fn test2(api: &webrender::RenderApi,
         width: u32,
         height: u32) -> webrender::StackingContext {

    let clip_rect = Rect::new(Point2D::new(0.0, 0.0), Size2D::new(width as f32, height as f32));
    let clip = ClipRegion::new(Rect::new(Point2D::new(0.0, 0.0), Size2D::new(2048.0, 2048.0)));

    let color_grey = ColorF::new(0.5, 0.5, 0.5, 1.0);
    let color_red = ColorF::new(0.5, 0.5, 0.0, 1.0);
    let color_green = ColorF::new(0.0, 0.5, 0.5, 1.0);
    let color_blue = ColorF::new(0.5, 0.0, 0.5, 1.0);

    let rect1 = Rect::new(Point2D::new(0.0, 0.0), Size2D::new(250.0, 250.0));
    let mut dl1 = DisplayListBuilder::new();
    dl1.push_rect(StackingLevel::Content, rect1, clip.clone(), color_red);
    let id1 = api.add_display_list(dl1, PipelineId(0), Epoch(0));

    let rect2 = Rect::new(Point2D::new(0.0, 0.0), Size2D::new(250.0, 250.0));
    let mut dl2 = DisplayListBuilder::new();
    dl2.push_rect(StackingLevel::Content, rect2, clip.clone(), color_green);
    let id2 = api.add_display_list(dl2, PipelineId(0), Epoch(0));

    let rect3 = Rect::new(Point2D::new(0.0, 0.0), Size2D::new(250.0, 250.0));
    let mut dl3 = DisplayListBuilder::new();
    dl3.push_rect(StackingLevel::Content, rect3, clip.clone(), color_blue);
    let id3 = api.add_display_list(dl3, PipelineId(0), Epoch(0));

    let mut sc0 = StackingContext::new(Some(webrender::ScrollLayerId::new(0)),
                                       Rect::new(Point2D::new(100.0, 100.0), Size2D::new(467.0, 462.0)),
                                       Rect::new(Point2D::new(0.0, 0.0), Size2D::new(467.0, 462.0)),
                                       0,
                                       &Matrix4::identity(),
                                       &Matrix4::identity(),
                                       false,
                                       webrender::MixBlendMode::Normal);

    let mut sc1 = StackingContext::new(None,
                                       Rect::new(Point2D::new(80.0, 60.0), Size2D::new(250.0, 250.0)),
                                       Rect::new(Point2D::new(0.0, 0.0), Size2D::new(250.0, 250.0)),
                                       0,
                                       &Matrix4::identity(),
                                       &Matrix4::identity(),
                                       false,
                                       webrender::MixBlendMode::Difference);
    sc1.add_display_list(id1);

    let mut sc2 = StackingContext::new(None,
                                       Rect::new(Point2D::new(140.0, 120.0), Size2D::new(250.0, 250.0)),
                                       Rect::new(Point2D::new(0.0, 0.0), Size2D::new(250.0, 250.0)),
                                       0,
                                       &Matrix4::identity(),
                                       &Matrix4::identity(),
                                       false,
                                       webrender::MixBlendMode::Difference);
    sc2.add_display_list(id2);

    let mut sc3 = StackingContext::new(None,
                                       Rect::new(Point2D::new(180.0, 180.0), Size2D::new(250.0, 250.0)),
                                       Rect::new(Point2D::new(0.0, 0.0), Size2D::new(250.0, 250.0)),
                                       0,
                                       &Matrix4::identity(),
                                       &Matrix4::identity(),
                                       false,
                                       webrender::MixBlendMode::Difference);
    sc3.add_display_list(id3);

    sc0.add_stacking_context(sc1);
    sc0.add_stacking_context(sc2);
    sc0.add_stacking_context(sc3);

    sc0
}

fn main() {
    let window = glutin::WindowBuilder::new()
                .with_gl_profile(glutin::GlProfile::Compatibility)
                .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGl, (2, 1)))
                .build()
                .unwrap();

    unsafe {
    	window.make_current().ok();
        gl::load_with(|symbol| window.get_proc_address(symbol));
        gl::clear_color(0.3, 0.0, 0.0, 1.0);
    }

    let version = unsafe {
        let data = CStr::from_ptr(gl::GetString(gl::VERSION) as *const _).to_bytes().to_vec();
        String::from_utf8(data).unwrap()
    };

    println!("OpenGL version {}", version);

    let (width, height) = window.get_inner_size().unwrap();

    let notifier = Box::new(Notifier::new(window.create_window_proxy()));

    let res_path = "/home/gw/code/work/servo_gfx/servo/resources";
    let mut renderer = renderer::Renderer::new(notifier, width, height, PathBuf::from(res_path));

    let api = renderer.new_api();

    //let sc = test1(&api, width, height);
    let sc = test2(&api, width, height);

    api.set_root_stacking_context(sc,
                                  ColorF::new(0.5, 0.5, 0.5, 1.0),
                                  Epoch(0),
                                  PipelineId(0));

    for event in window.wait_events() {
        //gl::clear(gl::COLOR_BUFFER_BIT);
        renderer.update();

        //let vis_rect = RectF::from_min_max(200.0, 100.0, 400.0, 200.0);
        //let vis_rect = Rect::new(Point2D::new(0.0, 0.0), Size2D::new(2048.0, 2048.0));
        renderer.render();
        //rc.render(&page, &vis_rect, width, height);

        window.swap_buffers().ok();

        match event {
            glutin::Event::Closed => break,
            glutin::Event::KeyboardInput(_element_state, scan_code, _virtual_key_code) => {
                if scan_code == 9 {
                    break;
                }
            }
            _ => ()
        }
    }
}
