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
const SAMPLES_PER_PIXEL: usize = 10;
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
	pub fn make_blocks(&mut self) -> Vec<ImageBlock> {
		let widths = self.width as usize / 20;
		let heights = self.height as usize / 20;

		let ptr = self.buffer.as_mut_ptr();

		let mut blocks = vec![];
		for block_id in 0..400 {
			let block_x = block_id % 20;
			let block_y = block_id / 20;

			let img_x = block_x * widths;
			let img_y = (block_y * heights) * self.width as usize;

			println!("{block_x} ({img_x}) {block_y} ({img_y})");

			let slice_start = img_x + img_y;

			let mut lines = vec![];
			for y in 0..heights {
				let offset = y * self.width as usize;

				unsafe {
					let start = ptr.add(slice_start + offset);
					let line = std::slice::from_raw_parts_mut(start, widths);
					lines.push(line);
				}
			}

			blocks.push(ImageBlock {
				width: widths as u16,
				height: heights as u16,
				x: block_x as u16 * widths as u16,
				y: block_y as u16 * heights as u16,
				lines,
			})
		}

		blocks
	}
}

pub struct ImageBlock {
	// The width of each line
	width: u16,
	// The number of lines
	height: u16,
	x: u16,
	y: u16,
	lines: Vec<&'static mut [u32]>,
}

impl ImageBlock {
	pub fn size(&self) -> usize {
		self.width as usize * self.height as usize
	}

	#[inline]
	pub fn set_pixel(&mut self, x: u16, y: u16, r: u8, g: u8, b: u8) {
		if x < self.width && y < self.height {
			self.lines[y as usize][x as usize] = Self::pack_pixel(r, g, b);
		}
	}

	#[inline]
	pub fn set_pixel_idx(&mut self, idx: usize, r: u8, g: u8, b: u8) {
		if idx < self.size() {
			let x = idx % self.width as usize;
			let y = idx / self.width as usize;
			//println!("{x} - {y}");
			self.lines[y][x] = Self::pack_pixel(r, g, b);
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
	block_pool: Arc<RwLock<Vec<ImageBlock>>>,
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
							for px in 0..block.size() {
								let x = (px & block.width as usize) + block.x as usize;
								let y = (px / block.width as usize) + block.y as usize;

								let traced = renderer.pixel(x, y);

								block.set_pixel_idx(px, traced.r, traced.g, traced.b)
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

	pub fn get_block(&self) -> Option<ImageBlock> {
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
	let jobs: Arc<RwLock<Vec<ImageBlock>>> = Arc::new(RwLock::new(vec![]));

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

/*
fn render_threaded_lines(renderer: Renderer) -> Vec<u8> {
	let arc_renderer = Arc::new(renderer);
	let mut threads = vec![];

	for thread_num in 0..NUM_THREADS {
		let cloned = arc_renderer.clone();
		threads.push(thread::spawn(move || render_lines(cloned, thread_num)));
	}

	let mut component_vec = vec![0; IMAGE_WIDTH * IMAGE_HEIGHT * 3];

	//wait for all threads to finish execution, then fill the component vector
	for handle in threads {
		for (colours, row) in handle.join().unwrap() {
			for (row_index, component) in colours.into_iter().enumerate() {
				component_vec[(IMAGE_WIDTH * (IMAGE_HEIGHT - 1 - row)) * 3 + row_index] = component;
			}
		}
	}

	println!("\rFinished rendering!                         ");
	//todo: why does this still print the last number?
	//gen: because the number starts at col 20 in the `print!` and "Finished rendering!"
	//only takes 19 characters, leaving the space and number in place :D

	component_vec
}

fn render_lines(renderer: Arc<Renderer>, thread_num: usize) -> Vec<(Vec<u8>, usize)> {
	let mut lines = Vec::with_capacity(IMAGE_HEIGHT / NUM_THREADS);
	for j in (0..IMAGE_HEIGHT).rev() {
		if j % NUM_THREADS != thread_num {
			continue;
		}
		print!("\rNow rendering line: {} ", j);
		stdout().flush().unwrap();
		lines.push((renderer.line(j), j));
	}
	lines
}
*/
