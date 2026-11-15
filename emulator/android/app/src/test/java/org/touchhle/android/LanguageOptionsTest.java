/* SPDX-License-Identifier: MPL-2.0 */
package org.touchhle.android;

import static org.junit.Assert.*;
import android.content.Context;
import android.os.Looper;
import android.widget.Spinner;
import java.util.*;
import org.junit.*;
import org.junit.runner.RunWith;
import org.robolectric.*;
import org.robolectric.android.controller.ActivityController;
import org.robolectric.annotation.Config;

@RunWith(RobolectricTestRunner.class)
@Config(sdk={26,35})
public class LanguageOptionsTest {
    @Test public void existingInstallWithoutLanguageChoiceDefaultsToEnglish() {
        Context context = RuntimeEnvironment.getApplication();
        context.getSharedPreferences("language", Context.MODE_PRIVATE).edit().clear().commit();
        List<String> arguments = new ArrayList<>();
        LanguageOptions.appendArguments(context, arguments);
        assertEquals(Collections.singletonList("--preferred-languages=en"), arguments);
    }

    @Test public void dropdownPersistsEachChoiceAcrossRecreationAndFeedsGameArguments() {
        try (ActivityController<LauncherActivity> controller =
                Robolectric.buildActivity(LauncherActivity.class).setup()) {
            String[] codes = {"en", "de", "fr", "es", "it", "ja", "system"};
            for (int i = 0; i < codes.length; i++) {
                Spinner spinner = controller.get().getWindow().getDecorView()
                    .findViewWithTag(LanguageOptions.CONTROL_TAG);
                assertNotNull(spinner);
                if (i == 0) assertEquals("English", spinner.getSelectedItem());
                spinner.setSelection(i);
                Shadows.shadowOf(Looper.getMainLooper()).idle();
                controller.recreate();
                Shadows.shadowOf(Looper.getMainLooper()).idle();
                Spinner restored = controller.get().getWindow().getDecorView()
                    .findViewWithTag(LanguageOptions.CONTROL_TAG);
                assertEquals(i, restored.getSelectedItemPosition());
                List<String> arguments = new ArrayList<>();
                LanguageOptions.appendArguments(controller.get(), arguments);
                assertEquals("system".equals(codes[i]) ? Collections.emptyList()
                    : Collections.singletonList("--preferred-languages=" + codes[i]), arguments);
            }
        }
    }

    @Test public void invalidPreferenceFallsBackToEnglish() {
        Context context = RuntimeEnvironment.getApplication();
        context.getSharedPreferences("language", Context.MODE_PRIVATE).edit()
            .putString("code", "--unapproved-option").commit();
        List<String> arguments = new ArrayList<>();
        LanguageOptions.appendArguments(context, arguments);
        assertEquals(Collections.singletonList("--preferred-languages=en"), arguments);
    }
}
