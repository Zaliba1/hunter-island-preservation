/* SPDX-License-Identifier: MPL-2.0 */
import org.touchhle.android.IpaInspector;
public class InspectorCLI {
    public static void main(String[] args) {
        try {
            IpaInspector.Info info=IpaInspector.inspect(new java.io.File(args[0]));
            System.out.println(info.version+" "+info.minimumIOS+" "+info.sha256);
        }catch(Exception error){System.err.println(error.getMessage());System.exit(2);}
    }
}
