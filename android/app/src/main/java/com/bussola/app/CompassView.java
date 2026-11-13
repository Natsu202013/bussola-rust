package com.bussola.app;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.hardware.Sensor;
import android.hardware.SensorEvent;
import android.hardware.SensorEventListener;
import android.hardware.SensorManager;
import android.view.View;

import com.bussola.core.Core;

/**
 * Draws the compass rose with a plain {@link Canvas}: no images, no libraries.
 *
 * <p>Orientation comes from the rotation-vector sensor (fallback:
 * accelerometer + magnetometer), is smoothed by the Rust core and rendered as a
 * rotating dial plus a fixed needle.
 */
public final class CompassView extends View implements SensorEventListener {

    private static final int BG = 0xFF05070A;
    private static final int DIAL = 0xFF11161F;
    private static final int RING = 0xFF1E2633;
    private static final int TICK_MINOR = 0xFF2C3644;
    private static final int TICK_MAJOR = 0xFF6B7889;
    private static final int TEXT = 0xFFE8EDF5;
    private static final int TEXT_DIM = 0xFF7D8A9C;
    private static final int ACCENT = 0xFFFF5A4E;
    private static final int ACCENT_SOFT = 0xFF3A2020;

    private static final String[] CARDINALS =
            {"N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE",
             "S", "SSW", "SW", "WSW", "W", "WNW", "NW", "NNW"};

    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path path = new Path();
    private final RectF arc = new RectF();

    private final SensorManager sensors;
    private final Sensor rotationVector;
    private final Sensor accelerometer;
    private final Sensor magnetometer;

    private Core core;
    private float heading;
    private boolean hasSensor;
    private String status = "Starting sensors";

    private final float[] rv = new float[9];
    private final float[] rvOut = new float[9];
    private final float[] orientation = new float[3];
    private final float[] accel = new float[3];
    private final float[] mag = new float[3];
    private boolean haveAccel;
    private boolean haveMag;
    private long lastRenderNs;

    private float textSm, textCardinal, textDeg;

    public CompassView(Context context) {
        super(context);
        core = new Core();
        sensors = (SensorManager) context.getSystemService(Context.SENSOR_SERVICE);
        rotationVector = sensors == null
                ? null : sensors.getDefaultSensor(Sensor.TYPE_ROTATION_VECTOR);
        accelerometer = sensors == null
                ? null : sensors.getDefaultSensor(Sensor.TYPE_ACCELEROMETER);
        magnetometer = sensors == null
                ? null : sensors.getDefaultSensor(Sensor.TYPE_MAGNETIC_FIELD);
        setBackgroundColor(BG);
        setLayerType(View.LAYER_TYPE_HARDWARE, null);
    }

    void start() {
        if (sensors == null) {
            status = "No sensor service";
            invalidate();
            return;
        }
        hasSensor = false;
        if (rotationVector != null) {
            hasSensor = sensors.registerListener(
                    this, rotationVector, SensorManager.SENSOR_DELAY_GAME) || hasSensor;
        }
        if (!hasSensor && accelerometer != null && magnetometer != null) {
            hasSensor = sensors.registerListener(
                    this, accelerometer, SensorManager.SENSOR_DELAY_GAME) || hasSensor;
            hasSensor = sensors.registerListener(
                    this, magnetometer, SensorManager.SENSOR_DELAY_GAME) || hasSensor;
        }
        status = hasSensor ? null : "No compass sensor";
        invalidate();
    }

    void stop() {
        if (sensors != null) {
            sensors.unregisterListener(this);
        }
    }

    void release() {
        stop();
        if (core != null) {
            core.close();
            core = null;
        }
    }

    @Override
    public void onSensorChanged(SensorEvent event) {
        switch (event.sensor.getType()) {
            case Sensor.TYPE_ROTATION_VECTOR:
                SensorManager.getRotationMatrixFromVector(rv, event.values);
                SensorManager.getOrientation(rv, orientation);
                // atan2 azimuth in radians -> the Rust core handles the shift.
                push(orientation[0], true);
                return;
            case Sensor.TYPE_ACCELEROMETER:
                System.arraycopy(event.values, 0, accel, 0, 3);
                haveAccel = true;
                break;
            case Sensor.TYPE_MAGNETIC_FIELD:
                System.arraycopy(event.values, 0, mag, 0, 3);
                haveMag = true;
                break;
            default:
                return;
        }
        if (haveAccel && haveMag) {
            float[] r = new float[9];
            if (SensorManager.getRotationMatrix(r, null, accel, mag)) {
                SensorManager.getOrientation(r, orientation);
                push(orientation[0], true);
            }
        }
    }

    private void push(float value, boolean radians) {
        if (core == null) {
            return;
        }
        heading = radians ? core.updateRadians(value) : core.update(value);
        // Cap redraws near 60 Hz: sensors can fire faster than the screen.
        long now = System.nanoTime();
        if (now - lastRenderNs > 15_000_000L) {
            lastRenderNs = now;
            postInvalidateOnAnimation();
        }
    }

    @Override
    public void onAccuracyChanged(Sensor sensor, int accuracy) {
        // Nothing to do: smoothing already tolerates noisy samples.
    }

    @Override
    protected void onSizeChanged(int w, int h, int ow, int oh) {
        super.onSizeChanged(w, h, ow, oh);
        float min = Math.min(w, h);
        textSm = min * 0.036f;
        textCardinal = min * 0.085f;
        textDeg = min * 0.13f;
    }

    @Override
    protected void onDraw(Canvas canvas) {
        int w = getWidth();
        int h = getHeight();
        if (w == 0 || h == 0) {
            return;
        }
        float cx = w * 0.5f;
        float cy = h * 0.5f;
        float radius = Math.min(w, h) * 0.40f;

        canvas.drawColor(BG);

        // --- dial backdrop -------------------------------------------------
        paint.reset();
        paint.setAntiAlias(true);
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(DIAL);
        canvas.drawCircle(cx, cy, radius, paint);

        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(Math.max(1f, radius * 0.006f));
        paint.setColor(RING);
        canvas.drawCircle(cx, cy, radius, paint);
        canvas.drawCircle(cx, cy, radius * 0.70f, paint);

        // --- ticks (rotate the whole rose by -heading) ---------------------
        canvas.save();
        canvas.rotate(-heading, cx, cy);

        for (int deg = 0; deg < 360; deg += 5) {
            boolean major = deg % 30 == 0;
            boolean cardinal = deg % 45 == 0;
            float len = radius * (cardinal ? 0.11f : major ? 0.08f : 0.045f);
            paint.setColor(cardinal ? TICK_MAJOR : TICK_MINOR);
            paint.setStrokeWidth(Math.max(1f, radius * (cardinal ? 0.010f : 0.006f)));
            double rad = Math.toRadians(deg);
            float sin = (float) Math.sin(rad);
            float cos = (float) Math.cos(rad);
            float r1 = radius * 0.985f;
            float r2 = r1 - len;
            canvas.drawLine(cx + sin * r1, cy - cos * r1,
                    cx + sin * r2, cy - cos * r2, paint);
        }

        // --- cardinal / intercardinal labels ------------------------------
        paint.setStyle(Paint.Style.FILL);
        paint.setTextAlign(Paint.Align.CENTER);
        paint.setTypeface(Typeface.create(Typeface.SANS_SERIF, Typeface.NORMAL));

        for (int i = 0; i < 16; i += 2) {          // 22.5° steps
            float deg = i * 22.5f;
            boolean cardinal = i % 4 == 0;
            paint.setTextSize(cardinal ? textCardinal : textSm);
            paint.setColor(cardinal ? TEXT : TEXT_DIM);
            paint.setFakeBoldText(cardinal);
            String label = CARDINALS[i];
            double rad = Math.toRadians(deg);
            float r = radius * 0.80f;
            float x = cx + (float) Math.sin(rad) * r;
            float y = cy - (float) Math.cos(rad) * r
                    + (paint.getTextSize() * 0.35f);
            canvas.drawText(label, x, y, paint);
        }
        paint.setFakeBoldText(false);
        canvas.restore();

        // --- centre readout ------------------------------------------------
        paint.setStyle(Paint.Style.FILL);
        paint.setTextAlign(Paint.Align.CENTER);
        paint.setColor(ACCENT);
        paint.setTextSize(textCardinal);
        paint.setFakeBoldText(true);
        String cardinal = Core.cardinal(heading);
        canvas.drawText(cardinal, cx, cy + textCardinal * 0.34f, paint);
        paint.setFakeBoldText(false);

        paint.setColor(TEXT_DIM);
        paint.setTextSize(textSm * 0.95f);
        canvas.drawText(Math.round(heading) % 360 + "°", cx, cy - textCardinal * 0.75f, paint);

        // --- needle (fixed, pointing up = where you are heading) -----------
        float needleLen = radius * 0.62f;
        float needleW = radius * 0.055f;
        drawNeedle(canvas, cx, cy - needleLen * 0.10f, needleLen, needleW, ACCENT);
        drawNeedle(canvas, cx, cy + needleLen * 0.10f, needleLen * 0.72f, needleW, ACCENT_SOFT);

        // --- status line ----------------------------------------------------
        if (status != null) {
            paint.setColor(TEXT_DIM);
            paint.setTextSize(textSm);
            canvas.drawText(status, cx, cy + radius * 1.30f, paint);
        } else {
            paint.setColor(TEXT_DIM);
            paint.setTextSize(textSm * 0.9f);
            canvas.drawText("Rust core · " + Math.round(heading) + "°",
                    cx, cy + radius * 1.30f, paint);
        }
    }

    /** North half of the needle: a slim triangle with a soft inner shadow. */
    private void drawNeedle(Canvas canvas, float cx, float tipY, float len,
                            float halfWidth, int color) {
        boolean up = color == ACCENT;
        path.reset();
        if (up) {
            path.moveTo(cx, tipY - len);
            path.lineTo(cx + halfWidth, tipY);
            path.lineTo(cx - halfWidth, tipY);
        } else {
            path.moveTo(cx, tipY + len);
            path.lineTo(cx + halfWidth * 0.8f, tipY);
            path.lineTo(cx - halfWidth * 0.8f, tipY);
        }
        path.close();
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(color);
        canvas.drawPath(path, paint);
    }
}
