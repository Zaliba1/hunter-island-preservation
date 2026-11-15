/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * Parts of this file are derived from SDL 2's Android project template, which
 * has a different license. Please see vendor/SDL/LICENSE.txt for details.
 */
package org.touchhle.android;

import org.libsdl.app.SDLActivity;

/**
 * A wrapper class over SDLActivity
 */

public class MainActivity extends SDLActivity {
    static boolean gameThreadRunning() {
        Thread thread=mSDLThread;
        return thread!=null && thread.isAlive();
    }
    @Override
    protected String[] getArguments() {
        // This activity is not exported. Only the inspected private import is used.
        String game = getIntent().getStringExtra("game");
        if (game == null || !new java.io.File(game).isFile())
            throw new IllegalStateException("Bitte das Spiel über den Importlauncher öffnen");
        String profile=getSharedPreferences("display",MODE_PRIVATE).getString("family","iphone-retina");
        String family="ipad".equals(profile)?"ipad":"iphone";
        java.util.ArrayList<String> arguments=new java.util.ArrayList<>(java.util.Arrays.asList(game,"--no-error-popup","--experimental-gles2",
            "--legacy-string-unsigned-int","--legacy-deferred-texture-deletes",
            "--landscape-left","--device-family="+family));
        if("iphone-retina".equals(profile))arguments.add("--experimental-retina");
        LanguageOptions.appendArguments(this, arguments);
        ModOptions.appendArguments(this, arguments);
        return arguments.toArray(new String[0]);
    }

    @Override
    protected String[] getLibraries() {
        return new String[]{
            "SDL2",
            "touchHLE"
        };
    }
}
