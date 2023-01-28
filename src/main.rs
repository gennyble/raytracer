use std::{
	cell::UnsafeCell,
	fs::File,
	io::{stdout, Write},
	ops::{Deref, DerefMut},
	path::Path,
	sync::{
		atomic::{AtomicBool, Ordering},
		RwLock, RwLockReadGuard,
	},
	thread::JoinHandle,
	time::Duration,
};
use std::{io::BufWriter, sync::Arc};
use std::{thread, time::Instant};

use png::Encoder;

use rand::seq::SliceRandom;
use raytracer::{worlds::*, Renderer, Viewport};
use winit::{
	event::{Event, WindowEvent},
	event_loop::{ControlFlow, EventLoop, EventLoopProxy},
	window::{Window, WindowBuilder},
};

//Image parameters
const SAMPLES_PER_PIXEL: usize = 2;
const MAX_DEPTH: usize = 10;

pub struct Image {
	width: u16,
	height: u16,
	buffer: Vec<u32>,
}

impl Image {
	pub fn new(width: u16, height: u16) -> Self {
		Self {
			width,
			height,
			buffer: vec![0; width as usize * height as usize],
		}
	}

	pub fn resize(&mut self, width: u16, height: u16) {
		self.width = width;
		self.height = height;
		let resize_size = width as usize * height as usize;

		self.buffer.resize(resize_size, 0);
		self.buffer.fill(0);
	}

	pub fn buffer(&self) -> &[u32] {
		&self.buffer
	}

	pub fn width(&self) -> u16 {
		self.width
	}

	pub fn height(&self) -> u16 {
		self.height
	}

	/// The [ImageBlock] that this function returns are wildely unsafe, lol. Do not let this struct
	/// drop before they do.
	pub fn make_blocks(&mut self) -> Vec<ImageLine> {
		let ptr = self.buffer.as_mut_ptr();

		let mut lines = vec![];
		for y in 0..self.height {
			let slice_start = y as usize * self.width as usize;

			let line = unsafe {
				let start = ptr.add(slice_start);
				std::slice::from_raw_parts_mut(start, self.width as usize)
			};

			lines.push(ImageLine {
				width: self.width,
				y: self.height - 1 - y,
				line,
			})
		}

		lines
	}
}

pub struct ImageLine {
	// The width of the line
	width: u16,
	// Which line
	y: u16,
	line: &'static mut [u32],
}

impl ImageLine {
	#[inline]
	pub fn set_pixel(&mut self, x: u16, r: u8, g: u8, b: u8) {
		if x < self.width {
			self.line[x as usize] = Self::pack_pixel(r, g, b);
		}
	}

	#[inline(always)]
	pub fn pack_pixel(r: u8, g: u8, b: u8) -> u32 {
		let mut pixel = 0u32 | b as u32 | (g as u32) << 8 | (r as u32) << 16;

		pixel
	}
}

#[derive(Clone)]
pub struct BlockSaftey {
	pub slices_valid_lock: Arc<RwLock<bool>>,
}

impl BlockSaftey {
	pub fn new() -> Self {
		Self {
			slices_valid_lock: Arc::new(RwLock::new(false)),
		}
	}

	pub fn invalidate_slices(&self) {
		*self.slices_valid_lock.write().unwrap() = false;
	}

	pub fn validate_slices(&self) {
		*self.slices_valid_lock.write().unwrap() = true;
	}
}

pub struct Worker {
	elp: EventLoopProxy<()>,
	renderer: Arc<RwLock<Option<Renderer>>>,
	block_saftey: BlockSaftey,
	block_pool: Arc<RwLock<Vec<ImageLine>>>,
}

impl Worker {
	pub fn spawn(self) -> JoinHandle<()> {
		std::thread::spawn(move || self.work())
	}

	pub fn write_safe(&self) -> RwLockReadGuard<bool> {
		self.block_saftey.slices_valid_lock.read().unwrap()
	}

	pub fn work(self) {
		loop {
			match self.get_block() {
				None => (),
				Some(mut block) => {
					if *self.write_safe().deref() {
						let renderer = self.renderer.read().unwrap();
						if let Some(renderer) = renderer.deref() {
							println!("Rendering");
							for x in 0..block.width {
								let traced = renderer.pixel(x as usize, block.y as usize);

								block.set_pixel(x, traced.r, traced.g, traced.b)
							}

							println!("Sent redraw");
							self.elp.send_event(()).unwrap();
						}
					}
				}
			}

			std::thread::sleep(Duration::from_millis(100));
		}
	}

	pub fn get_block(&self) -> Option<ImageLine> {
		if *self.write_safe().deref() {
			let mut pool_lock = self.block_pool.write().unwrap();
			return pool_lock.pop();
		}

		None
	}
}

fn main() {
	let event_loop = EventLoop::new();
	let window = WindowBuilder::new().build(&event_loop).unwrap();

	let context = unsafe { softbuffer::Context::new(&window) }.unwrap();
	let mut surface = unsafe { softbuffer::Surface::new(&context, &window) }.unwrap();

	let block_saftey = BlockSaftey::new();
	let mut image: Option<Image> = None;
	let mut renderer: Arc<RwLock<Option<Renderer>>> = Arc::new(RwLock::new(None));
	let jobs: Arc<RwLock<Vec<ImageLine>>> = Arc::new(RwLock::new(vec![]));

	let count = std::thread::available_parallelism().unwrap();
	println!("Available Parallelism: {count}");
	let mut handles = vec![];

	for _ in 0..count.get() {
		let worker = Worker {
			elp: event_loop.create_proxy(),
			renderer: renderer.clone(),
			block_saftey: block_saftey.clone(),
			block_pool: jobs.clone(),
		};

		handles.push(worker.spawn())
	}

	event_loop.run(move |event, _, control_flow| {
		*control_flow = ControlFlow::Wait;

		match event {
			Event::RedrawRequested(_window_id) => {
				match image.as_ref() {
					None => (),
					Some(img) => surface.set_buffer(img.buffer(), img.width(), img.height()),
				}
				println!("drawn");
			}
			Event::WindowEvent {
				event: WindowEvent::CloseRequested,
				..
			} => {
				*control_flow = ControlFlow::Exit;
			}
			Event::UserEvent(()) => window.request_redraw(),
			Event::WindowEvent {
				event: WindowEvent::Resized(size),
				..
			} => {
				// It looks like resize is called at creation as well as when resized,
				// so we can get the size here after the window is made.

				let mut job_lock = jobs.write().unwrap();
				// When we resize the image, some slices will be invalid. Block until nothing is writing to them.
				block_saftey.invalidate_slices();

				match image.as_mut() {
					None => image = Some(Image::new(size.width as u16, size.height as u16)),
					Some(img) => img.resize(size.width as u16, size.height as u16),
				}

				let mut blocks = image.as_mut().unwrap().make_blocks();
				blocks.shuffle(&mut rand::thread_rng());
				job_lock.clear();
				job_lock.extend(blocks);

				let aspect = size.width as f64 / size.height as f64;
				let (world, camera) = complex_random_scene(aspect);
				let viewport = Viewport::new(
					size.width as usize,
					size.height as usize,
					SAMPLES_PER_PIXEL,
					MAX_DEPTH,
				);
				let created_renderer = Renderer::new(viewport, camera, world);
				let mut render_lock = renderer.write().unwrap();
				*render_lock.deref_mut() = Some(created_renderer);

				block_saftey.validate_slices();
				println!("Resize reset!");
			}
			_ => (),
		}
	});
}
