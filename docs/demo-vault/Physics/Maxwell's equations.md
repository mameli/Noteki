---
tags: [physics, electromagnetism]
source: Feynman Lectures, vol. II
---
# Maxwell's equations

Four equations hold all of classical electromagnetism[^1]. In differential form, with the charge density $\rho$ and the current density $\mathbf{J}$:

$$
\begin{aligned}
\nabla \cdot \mathbf{E} &= \frac{\rho}{\varepsilon_0} \\
\nabla \cdot \mathbf{B} &= 0 \\
\nabla \times \mathbf{E} &= -\frac{\partial \mathbf{B}}{\partial t} \\
\nabla \times \mathbf{B} &= \mu_0 \mathbf{J} + \mu_0 \varepsilon_0 \frac{\partial \mathbf{E}}{\partial t}
\end{aligned}
$$

> [!tip] Light is a wave of the field
> In empty space they give the wave equation, with a speed $c = 1/\sqrt{\mu_0 \varepsilon_0}$:
> $$\nabla^2 \mathbf{E} = \frac{1}{c^2} \frac{\partial^2 \mathbf{E}}{\partial t^2}$$

## The same, in integral form

Gauss's law over a closed surface $S$ enclosing the charge $Q$: $\oint_S \mathbf{E} \cdot d\mathbf{A} = Q/\varepsilon_0$. And Faraday's law around a loop $C$:

$$\oint_C \mathbf{E} \cdot d\boldsymbol{\ell} = -\frac{d}{dt} \int_S \mathbf{B} \cdot d\mathbf{A}$$

| Law | Says | Unit of the field |
| --- | --- | --- |
| Gauss | charges are sources of $\mathbf{E}$ | V/m |
| Gauss for magnetism | no magnetic monopoles | T |
| Faraday | a changing $\mathbf{B}$ makes $\mathbf{E}$ curl | V/m |
| Ampère–Maxwell | currents and a changing $\mathbf{E}$ make $\mathbf{B}$ curl | T |

See also [[Euler's identity]] and the [[Reading list]].

[^1]: Written in this form by Oliver Heaviside in 1884, from Maxwell's twenty equations of 1865.
