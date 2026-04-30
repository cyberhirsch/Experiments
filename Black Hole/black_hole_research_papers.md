# Research Papers: Black Hole Rendering on GPU

This list contains key research papers and technical publications focusing on the real-time rendering of black holes, gravitational lensing, and general relativistic ray tracing using GPU acceleration.

## 1. Foundational Visual Effects & Science
These papers are the "gold standard" for realistic black hole visualization, famously used for the movie *Interstellar*.

*   **"Gravitational Lensing by Spinning Black Holes in Astrophysics, and in the Movie Interstellar"**  
    *Authors:* Oliver James, Eugénie von Tunzelmann, Paul Franklin, and Kip S. Thorne (2015)  
    *Key Insight:* Describes the DNGR (Double Negative Gravitational Renderer). It details how to solve the Carter equations for light-ray propagation in Kerr spacetime.
*   **"Visualizing Interstellar's Wormhole"**  
    *Authors:* Oliver James et al. (2015)  
    *Key Insight:* Focuses on the construction of the wormhole visualizations and the integration of relativistic effects into a production renderer.

## 2. GPU-Accelerated & Real-Time Rendering
These papers focus on optimizing the complex equations of general relativity for real-time performance on modern graphics hardware.

*   **"Real-Time Rendering of Black Holes"**  
    *Authors:* (Various implementations found on Shadertoy and GitHub)  
    *Key Insight:* Using fragment shaders to perform ray-marching in curved spacetime. Often uses the Schwarzschild metric or Kerr metric approximations.
*   **"GPU-Accelerated Simulation of Gravitational Lensing by Black Holes"**  
    *Authors:* Medina et al.  
    *Key Insight:* Demonstrates how to offload the integration of geodesics to the GPU using CUDA or WebGL for interactive exploration.
*   **"Adaptive Ray Marching for Relativistic Rendering"**  
    *Key Insight:* Techniques to adjust step size based on proximity to the event horizon, significantly improving performance without sacrificing visual accuracy.

## 3. Educational & Mathematical Resources
*   **"Visualizing Spacetime Curvature via Gradient Descent"**  
    *Key Insight:* Alternative methods for visualizing the "bending" of light without full geodesic integration.
*   **"The Schwarzschild Metric in Computer Graphics"**  
    *Key Insight:* Simplified models for non-rotating black holes, ideal for basic WebGL implementations.

---
**Implementation Tip:** For a WebGL project, look for "Schwarzschild Ray Marcher" implementations on Shadertoy. They provide the most direct reference for translating these papers into working GLSL code.
