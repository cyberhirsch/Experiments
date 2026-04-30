# Research Papers: Solar Corona & Magnetohydrodynamics (MHD)

This list contains key research papers and publications that provide the mathematical and computational foundations for simulating the solar corona, magnetic loops, and Coronal Mass Ejections (CMEs) using fluid dynamics.

## 1. Computer Graphics & Fluid Simulation
These papers focus on visual plausibility and efficient solvers suitable for animation and interactive systems.

*   **"On the Accurate Large-scale Simulation of Ferrofluids"**  
    *Authors:* Huang et al. (SIGGRAPH 2019)  
    *Key Insight:* Detailed simulation of magnetic fluids using SPH. Introduces "smooth magnets" and bounded magnetic force formulas to avoid instabilities.
*   **"A Magnetohydrodynamic Solver for Fluid Simulation"**  
    *Authors:* (Various, often appearing in Eurographics or SCA)  
    *Key Insight:* Adapts standard grid-based fluid solvers to include the induction equation and Lorentz force.
*   **"Simulating Magnetic Liquids"**  
    *Authors:* Moriguchi & Kondo (2004)  
    *Key Insight:* Early work on integrating magnetic forces into particle-based fluid simulations.

## 2. Astrophysical & Classical MHD
These are foundational scientific papers for the physics of the Sun and high-fidelity MHD solvers.

*   **"The FLIP Method for Magnetohydrodynamics"**  
    *Authors:* J.U. Brackbill & H.M. Ruppel (1986)  
    *Key Insight:* The original application of the Fluid-Implicit-Particle (FLIP) method to magnetohydrodynamics. Explains how particles carry magnetic flux.
*   **"Smoothed Particle Magnetohydrodynamics: A review"**  
    *Authors:* Daniel J. Price (2012)  
    *Key Insight:* A comprehensive overview of implementing MHD in particle systems (SPH), focusing on the divergence-free constraint ($\nabla \cdot \mathbf{B} = 0$).
*   **"Three-dimensional magnetohydrodynamic simulation of coronal mass ejections"**  
    *Authors:* Manchester et al. (2004)  
    *Key Insight:* Scientific simulation of CMEs using high-fidelity MHD, detailing the magnetic reconnection processes.

## 3. Mathematical Foundations
*   **"Magnetohydrodynamics on a Staggered Grid"**  
    *Key Insight:* Discusses the Yee-grid approach to ensuring $\nabla \cdot \mathbf{B} = 0$ in numerical simulations.
*   **"The Induction Equation in Ideal MHD"**  
    *Key Insight:* Describes how the magnetic field is "frozen into" the fluid flow, which is the basis for most solar loop advection models.

---
**Note:** For the `corona.html` project, the **Brackbill (1986)** paper is the most relevant for your FLIP-based approach, while the **Huang (2019)** paper provides the best modern "graphics" optimizations.
