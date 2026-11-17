/* SPDX-License-Identifier: MPL-2.0 */
package org.touchhle.android;

import android.content.Context;
import android.net.Uri;
import org.json.JSONObject;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.nio.channels.*;
import java.util.concurrent.Callable;
import java.util.UUID;

/** Private imports; no scanning of another app's files or existing saves. */
final class GameStore {
    private final Context context;
    private final File games, settings;
    GameStore(Context context) throws IOException {
        this.context=context;
        games=new File(context.getFilesDir(),"games");settings=new File(context.getFilesDir(),"import.json");
        if(!games.isDirectory() && !games.mkdirs())throw new IOException("App-Speicher nicht verfuegbar");
    }
    JSONObject settings() throws Exception {
        if(!settings.isFile())return new JSONObject();
        if(settings.length()>16384)throw new IOException("Importdaten beschaedigt");
        return new JSONObject(new String(Files.readAllBytes(settings.toPath()),StandardCharsets.UTF_8));
    }
    IpaInspector.Info importGame(Uri uri) throws Exception {
        return exclusive(()->importLocked(uri));
    }
    <T> T exclusive(Callable<T> action) throws Exception {
        try(RandomAccessFile file=new RandomAccessFile(new File(context.getFilesDir(),"import.lock"),"rw");FileChannel channel=file.getChannel()) {
            FileLock acquired;
            try {acquired=channel.tryLock();}catch(OverlappingFileLockException busy){throw new IOException("Ein Import oder Startvorgang läuft bereits.");}
            if(acquired==null)throw new IOException("Ein Import oder Startvorgang läuft bereits.");
            try(FileLock held=acquired){return action.call();}
        }
    }
    private IpaInspector.Info importLocked(Uri uri) throws Exception {
        File temp=new File(games,UUID.randomUUID()+".partial");
        try {
            try(InputStream in=context.getContentResolver().openInputStream(uri);OutputStream out=new FileOutputStream(temp)) {
                if(in==null)throw new IOException("Datei konnte nicht geoeffnet werden");
                long size=0;byte[] buffer=new byte[65536];int n;
                while((n=in.read(buffer))!=-1){size+=n;if(size>IpaInspector.MAX_ARCHIVE)throw new IOException("IPA zu gross");out.write(buffer,0,n);}
            }
            IpaInspector.Info info=IpaInspector.inspect(temp);
            File dest=new File(games,info.sha256+".ipa");
            if(dest.exists()) {
                if(!IpaInspector.digest(dest).equals(info.sha256))throw new IOException("Vorhandene Kopie beschaedigt");
            } else Files.move(temp.toPath(),dest.toPath(),StandardCopyOption.ATOMIC_MOVE);
            JSONObject data=new JSONObject();data.put("bundle",IpaInspector.BUNDLE);data.put("version",info.version);
            data.put("minimum_ios",info.minimumIOS);data.put("sha256",info.sha256);
            File staging=new File(settings.getParentFile(),"import-"+UUID.randomUUID()+".partial");
            try {
                Files.write(staging.toPath(),data.toString(2).getBytes(StandardCharsets.UTF_8));
                Files.move(staging.toPath(),settings.toPath(),StandardCopyOption.ATOMIC_MOVE,StandardCopyOption.REPLACE_EXISTING);
            } finally {staging.delete();}
            return info;
        } finally {temp.delete();}
    }
    File game() throws Exception {
        return exclusive(this::verifiedGame);
    }
    private File verifiedGame() throws Exception {
        if(MainActivity.gameThreadRunning())throw new IOException("Bitte zuerst das laufende Spiel vollständig beenden.");
        new SaveStore(context).recoverLocked();
        ModOptions.requireTestingSeed(context);
        String sha=settings().optString("sha256");
        if(!sha.matches("[0-9a-f]{64}"))throw new IOException("Bitte zuerst deine IPA importieren");
        File file=new File(games,sha+".ipa");
        if(!file.isFile() || !IpaInspector.digest(file).equals(sha))throw new IOException("Import beschaedigt; bitte erneut importieren");
        return file;
    }
    JSONObject diagnosticGame() throws Exception {
        JSONObject original=settings(),result=new JSONObject();
        if(!original.has("sha256"))return result;
        String sha=original.optString("sha256"),version=original.optString("version"),minimum=original.optString("minimum_ios");
        if(!sha.matches("[0-9a-f]{64}") || !version.matches("[0-9]+(?:\\.[0-9]+){0,5}") || !minimum.matches("[0-9]+(?:\\.[0-9]+){0,3}"))throw new IOException("Importdaten beschaedigt");
        result.put("bundle",IpaInspector.BUNDLE);result.put("version",version);result.put("minimum_ios",minimum);result.put("sha256",sha);
        return result;
    }
}
