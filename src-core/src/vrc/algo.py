import numpy as np
import matplotlib.pyplot as plt

def velocity_since(times, values, limit):
    """
    Calculate average absolute velocity (abs(delta value) / delta time) for all segments
    where both endpoints are after 'limit' timestamp.
    """
    sum_vel = 0.0
    count = 0.0
    for i in range(len(times) - 1):
        t_new, t_old = times[i], times[i+1]
        if t_new > limit:
            if t_old > limit:
                dv = abs(values[i] - values[i+1])
                dt = t_new - t_old
                if dt > 0:
                    sum_vel += dv / dt
                    count += 1
            else:
                # partial segment overlapping window
                dv = abs(values[i] - values[i+1])
                dt = t_new - t_old
                frac = (t_new - limit) / dt
                if (t_new - limit) > 0:
                    sum_vel += (dv * frac) / (t_new - limit)
                    count += 1
                break
        else:
            break
    return sum_vel / count if count > 0 else 0.0

# Simulation parameters
duration = 5.0            # total time in seconds
update_rate = 10.0        # input update rate in Hz
output_rate = 100.0       # output sampling rate in Hz
smoothing_time = 0.2      # smoothing window in seconds

# Weighting factors
position_weight = 0.3
velocity_weight = 1.0 - position_weight

dt_update = 1.0 / update_rate
dt_output = 1.0 / output_rate

# Generate update times and a ramp input: up in 2s, down in 3s
update_times = np.arange(0, duration + dt_update/2, dt_update)
peak_time = 2.0  # seconds to reach maximum
input_values = np.where(
    update_times <= peak_time,
    update_times / peak_time,
    np.maximum(0, (duration - update_times) / (duration - peak_time))
)

# Buffers for history
buffer_times = []
buffer_values = []

# Generate output timeline
output_times = np.arange(0, duration + dt_output/2, dt_output)
smoothed_velocities = []

# Simulate and compute smoothed absolute velocity
for t in output_times:
    # Record new sample on update ticks
    if np.isclose(update_times, t).any():
        idx = np.argmin(np.abs(update_times - t))
        buffer_times.insert(0, update_times[idx])
        buffer_values.insert(0, input_values[idx])
        # Cap history length to twice the smoothing window
        max_len = int(smoothing_time * update_rate * 2)
        buffer_times = buffer_times[:max_len]
        buffer_values = buffer_values[:max_len]

    # Compute averaged absolute velocity_since
    limit = t - smoothing_time
    vel = velocity_since(buffer_times, buffer_values, limit)
    smoothed_velocities.append(np.clip(vel, 0.0, 1.0))

# Sample commanded intensity (position) at output times
idxs = np.searchsorted(update_times, output_times, side='right') - 1
commanded = input_values[idxs]

# Combine position and velocity for final haptic output
combined_output = velocity_weight * np.array(smoothed_velocities) + position_weight * commanded

# Plot
plt.figure()
plt.plot(output_times, commanded, label='Position Component (20%)')
plt.plot(output_times, smoothed_velocities, linestyle='--', label='Velocity Component (80%)')
plt.plot(output_times, combined_output, linestyle=':', label='Combined Output')
plt.xlabel('Time (s)')
plt.ylabel('Value (0–1)')
plt.title('Position + Velocity Weighted Haptic Output')
plt.legend()
plt.grid(True)
plt.tight_layout()
plt.show()
