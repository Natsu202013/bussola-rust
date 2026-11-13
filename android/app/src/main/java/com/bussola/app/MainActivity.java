package com.bussola.app;

import android.os.Bundle;
import android.view.View;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.widget.FrameLayout;

/** Single-activity host: the whole UI is drawn by {@link CompassView}. */
public final class MainActivity extends android.app.Activity {

    private FrameLayout root;
    private CompassView compass;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        // Pure black background, no title bar: matching the drawing surface.
        Window window = getWindow();
        window.setBackgroundDrawable(new android.graphics.drawable.ColorDrawable(0xFF05070A));
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        if (android.os.Build.VERSION.SDK_INT >= 21) {
            window.setStatusBarColor(0xFF05070A);
            window.setNavigationBarColor(0xFF05070A);
        }
        requestWindowFeature(Window.FEATURE_NO_TITLE);

        if (android.os.Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController c = window.getInsetsController();
            if (c != null) {
                c.hide(WindowInsets.Type.systemBars());
                c.setSystemBarsBehavior(
                        WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        } else {
            //noinspection deprecation
            window.getDecorView().setSystemUiVisibility(
                    View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                            | View.SYSTEM_UI_FLAG_FULLSCREEN
                            | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY);
        }

        compass = new CompassView(this);
        root = new FrameLayout(this);
        root.setBackgroundColor(0xFF05070A);
        root.addView(compass, new FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT));
        setContentView(root);
    }

    @Override
    protected void onResume() {
        super.onResume();
        compass.start();
    }

    @Override
    protected void onPause() {
        compass.stop();
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        compass.release();
        super.onDestroy();
    }
}
