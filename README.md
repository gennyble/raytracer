# raytracer
A simple raytracer written in Rust, following [raytracing.github.io](https://raytracing.github.io)

![Complex raytraced scene, containing spheres of different materials, including glass, metal, and diffuse soft objects](https://pbs.twimg.com/media/E1-Sp5kVEAERqXR?format=jpg&name=large)

The book and associated code is focused on C++, so implementing this in Rust was a total rewrite and uses entirely different paradigms in many places.
Since I originally wrote this, versions of the book have been released in Rust; if you would like to repeat this journey they are a better pedagogical resource.
I enjoyed the challenge. This also expands on the book in several ways, including multithreading, multiple scenes, and PNG export.

Rendering takes a while. 
This doesn't leverage the GPU, so if you're using the complex scene at a high resolution and sample rate, be prepared to wait ~10 minutes on a decent CPU.

### notes from gennyble
this fork gives the raytracer a little UI in the form of a software-rendered window using [softbuffer][softbuffer-github].

it does terrible, terrible crimes that throw away a lot of the safety that Rust as a language provides. This is done so that the renderer can work on the same buffer i pass into softbuffer. i believe it to be safe, but i have not checked it nor tried too hard to verify it won't ever crash. it'll be fine; things can break sometimes. i encourage you to look at it, though! it's pretty interesting i think.

[softbuffer-github]: https://github.com/rust-windowing/softbuffer