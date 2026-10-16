// KoperCAD - "Export from EPLAN" helper script, script version 1.
//
// Written from scratch by KoperVonk. It contains no EPLAN content and no
// customer data. It runs inside YOUR OWN EPLAN, with YOUR OWN licence, as a
// normal EPLAN script (Utilities > Scripts). It needs no administrator
// rights and no EPLAN API licence: it only asks EPLAN to run its own
// "export" action (the same as the command line action
// "export /TYPE:PXFPROJECT"). It never changes the project and sends
// nothing anywhere.
//
// What you get, in Documents\KoperCAD\EPLAN-export\<project>-<date-time>\ :
//   project.epj          the EPLAN project exchange file (PXF, plain XML)
//   kopercad-export.xml  small manifest: script version, EPLAN version, date
//
// Steps are in KoperCAD (File > Import > Get the EPLAN export script) and in
// the README files next to this script.

using System;
using System.IO;
using System.Text;
using System.Threading;
using Eplan.EplApi.ApplicationFramework;
using Eplan.EplApi.Base;
using Eplan.EplApi.Scripting;

public class KoperCadExportFromEplan
{
    // Keep in step with KoperCAD (novo_interop::eplan::export_script).
    const int ScriptVersion = 1;

    // Optional: set this Windows environment variable to the project link
    // (.elk) you want exported. Leave it unset to export the open project.
    const string ProjectOverride = "KOPERCAD_EPLAN_PROJECT";

    // Give up waiting for the export file after this many seconds.
    const int WaitLimitSeconds = 1800;

    [Start]
    public void Run()
    {
        try
        {
            string folder = NewFolder();
            string file = Path.Combine(folder, "project.epj");

            ActionCallingContext context = new ActionCallingContext();
            context.AddParameter("TYPE", "PXFPROJECT");
            context.AddParameter("EXPORTFILE", file);
            string project = Environment.GetEnvironmentVariable(ProjectOverride);
            if (!string.IsNullOrEmpty(project))
            {
                context.AddParameter("PROJECTNAME", project);
            }

            new CommandLineInterpreter().Execute("export", context);

            if (!WaitForFile(file))
            {
                Tell("KoperCAD export: EPLAN did not produce project.epj.\n"
                    + "Is a project open? Folder: " + folder);
                return;
            }
            WriteManifest(folder, "project.epj", false);
            Tell("KoperCAD export finished.\n\nFolder:\n" + folder
                + "\n\nIn KoperCAD choose File > Import > EPLAN project and pick this folder.");
        }
        catch (Exception e)
        {
            Tell("KoperCAD export failed: " + e.Message);
        }
    }

    // Checks the folder, the permissions and the version stamp without
    // touching any project. Safe to run at any time.
    [DeclareAction("KoperCadSelfTest")]
    public void SelfTest()
    {
        try
        {
            string folder = NewFolder();
            File.WriteAllText(Path.Combine(folder, "selftest.txt"),
                "KoperCAD EPLAN export self-test, script version " + ScriptVersion + "\n");
            WriteManifest(folder, "", true);
            Tell("KoperCAD self-test passed.\n\nThe script runs and can write to:\n" + folder
                + "\n\nIn KoperCAD, import this folder to check the version.");
        }
        catch (Exception e)
        {
            Tell("KoperCAD self-test failed: " + e.Message);
        }
    }

    static string NewFolder()
    {
        string name = Safe(Substitute("$(PROJECTNAME)", "project"));
        string stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss");
        string root = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
            "KoperCAD", "EPLAN-export");
        string folder = Path.Combine(root, name + "-" + stamp);
        Directory.CreateDirectory(folder);
        return folder;
    }

    static string Substitute(string variable, string fallback)
    {
        try
        {
            string value = PathMap.SubstitutePath(variable);
            if (!string.IsNullOrEmpty(value) && !value.StartsWith("$("))
            {
                return value;
            }
        }
        catch (Exception)
        {
        }
        return fallback;
    }

    static string Safe(string text)
    {
        StringBuilder b = new StringBuilder();
        foreach (char c in text)
        {
            b.Append(Array.IndexOf(Path.GetInvalidFileNameChars(), c) >= 0 ? '_' : c);
        }
        string s = b.ToString().Trim().TrimEnd('.');
        return s.Length == 0 ? "project" : s;
    }

    // EPLAN may return before the file is complete: wait until its size has
    // stopped changing for three checks in a row.
    static bool WaitForFile(string file)
    {
        long last = -1;
        int steady = 0;
        for (int i = 0; i < WaitLimitSeconds; i++)
        {
            long size = File.Exists(file) ? new FileInfo(file).Length : 0;
            if (size > 0 && size == last)
            {
                steady++;
                if (steady >= 3)
                {
                    return true;
                }
            }
            else
            {
                steady = 0;
            }
            last = size;
            Thread.Sleep(1000);
        }
        return false;
    }

    static void WriteManifest(string folder, string pxf, bool selfTest)
    {
        string eplan = "";
        try
        {
            eplan = System.Diagnostics.Process.GetCurrentProcess()
                .MainModule.FileVersionInfo.ProductVersion;
        }
        catch (Exception)
        {
        }
        string xml = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n"
            + "<KoperCadEplanExport scriptVersion=\"" + ScriptVersion + "\""
            + " eplanVersion=\"" + Esc(eplan) + "\""
            + " project=\"" + Esc(Substitute("$(PROJECTNAME)", "")) + "\""
            + " created=\"" + DateTime.UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ") + "\""
            + " pxfFile=\"" + Esc(pxf) + "\""
            + " selfTest=\"" + (selfTest ? "true" : "false") + "\"/>\n";
        File.WriteAllText(Path.Combine(folder, "kopercad-export.xml"), xml, new UTF8Encoding(false));
    }

    static string Esc(string s)
    {
        return s.Replace("&", "&amp;").Replace("<", "&lt;").Replace(">", "&gt;").Replace("\"", "&quot;");
    }

    static void Tell(string text)
    {
        try
        {
            new Decider().Decide(EnumDecisionType.eOkDecision, text, "KoperCAD",
                EnumDecisionReturn.eOK, EnumDecisionReturn.eOK);
        }
        catch (Exception)
        {
        }
    }
}
