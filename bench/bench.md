---
note: HIDDENWORD
---

# 1

The wake of a [cylinder](https://example.org/cylinder) is *unsteady* for $\beta = 43000$.

<!-- HIDDENWORD -->

## Background

We define $Re = U D / \nu$ and `n_max` is the step limit.

$$
\nabla \cdot \mathbf{u} = 0
$$

### Details

- First, set $x_i^2$.
- Then **compute** $\frac{\partial u}{\partial t}$.

## Method

```python
def step(u):
    return u + 1
```

![Vorticity in the wake](wake.png)

| Mesh | Elements |
|------|----------|
| A    | 1200     |
| B    | 2400     |

# Results

> The flow is steady below this limit.
