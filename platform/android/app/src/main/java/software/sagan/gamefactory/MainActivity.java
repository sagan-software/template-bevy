package software.sagan.gamefactory;

import android.view.View;
import android.os.Bundle;
import android.graphics.Insets;
import android.view.WindowInsets;

import com.google.androidgamesdk.GameActivity;

public class MainActivity extends GameActivity {
    static {
        System.loadLibrary("hello");
    }

    private static native void nativeInsets(float top, float right, float bottom, float left);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        View view = getWindow().getDecorView();
        view.setOnApplyWindowInsetsListener((v, insets) -> {
            Insets area = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            float density = getResources().getDisplayMetrics().density;
            nativeInsets(area.top / density, area.right / density, area.bottom / density, area.left / density);
            return insets;
        });
        view.requestApplyInsets();
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);

        if (hasFocus) {
            hideSystemUi();
        }
    }

    private void hideSystemUi() {
        View decorView = getWindow().getDecorView();
        decorView.setSystemUiVisibility(
                View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                        | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                        | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                        | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_FULLSCREEN
        );
    }
}