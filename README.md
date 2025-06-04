# rsview
A 3D model previewer written in Rust using wgpu as a research project.  

# 🧠 Purpose
I built this as a fast, minimal tool to preview 3D models without unneccesary overhead. The goal was not to make a feature-rich application, but something that could load and view a model in seconds-just click and view what it looks like. The final binary is around 15MB and starts near-instantly. 

# 🛠️ What it Does
* Loads, parses, and renders `.obj` and `.gLTF` 3D models (custom-built barebones parsers)
* Drag and drop, click to open, and command line support for faster opening
* Automatically fits the model within the viewport
* Rotates the model smoothly to get a surround view
* Utilizes wGPU for fast rendering
* Built entirely in Rust

# 📝 What I Learned
* Handling of different file types and integrating them within my code
* Fundamentals on low-level 3D rendering, such as vertex buffers, camera transforms, etc

# 🎯 End Result
Ultimately barebones (serves well as a giggle-worthy screensaver?), but I learned and achieved exactly what I wanted, and I'm more than happy with how it came out.  
<br>
![Cow of Doom](https://imgur.com/cTIw9jz.gif)  
*Model: “cow-nonormals” from [MIT CSAIL Sample .obj Files](https://groups.csail.mit.edu/graphics/classes/6.837/F03/models/cow-nonormals.obj)*
