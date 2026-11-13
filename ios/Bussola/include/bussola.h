/*
 * Public C surface of the Rust compass core.
 *
 * This header is hand-written and must stay in sync with the `#[no_mangle]`
 * functions in rust/src/lib.rs. It is also the only file the Swift bridging
 * header needs to import.
 */
#ifndef BUSSOLA_H
#define BUSSOLA_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque filter state; allocate with bussola_new, release with bussola_free. */
typedef struct Compass Compass;

/* Creates a compass. `alpha` is the smoothing factor in [0.02, 1.0]
 * (0.18 is a good default). Never returns NULL in practice, but a NULL
 * handle is handled gracefully by every other function. */
Compass *bussola_new(float alpha);

/* Releases a compass. Passing NULL is a no-op. */
void bussola_free(Compass *ptr);

/* Feeds a heading in degrees, returns the smoothed heading in [0, 360). */
float bussola_update(Compass *ptr, float raw_deg);

/* Feeds an atan2-style heading in radians (Android convention). */
float bussola_update_rad(Compass *ptr, float raw_rad);

/* Current smoothed heading in degrees. */
float bussola_heading(const Compass *ptr);

/* Constant correction added to every sample (magnetic declination). */
void bussola_set_offset(Compass *ptr, float deg);

/* Clears the smoothing state, keeping alpha and offset. */
void bussola_reset(Compass *ptr);

/* NUL-terminated cardinal label ("N", "NNE", ...) for a heading in degrees.
 * Static storage: do not free. */
const uint8_t *bussola_cardinal(float deg);

/* Number of rose sectors (always 16). */
int32_t bussola_cardinal_count(void);

/* NUL-terminated library version, e.g. "1.0.0". */
const uint8_t *bussola_version(void);

#ifdef __cplusplus
}
#endif

#endif /* BUSSOLA_H */
