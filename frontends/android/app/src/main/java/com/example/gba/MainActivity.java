package com.example.gba;

import android.annotation.SuppressLint;
import android.content.Intent;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.webkit.JavascriptInterface;
import androidx.annotation.RequiresApi;
import androidx.appcompat.app.AppCompatActivity;
import androidx.webkit.WebViewAssetLoader;
import androidx.webkit.WebViewClientCompat;
import android.os.Environment;
import android.app.DownloadManager;
import android.provider.MediaStore;
import android.content.ContentValues;
import android.net.Uri;
import android.widget.Toast;
import java.io.OutputStream;
import java.io.File;
import java.io.FileOutputStream;
import android.media.MediaScannerConnection;
import android.util.Base64;
import android.content.Context;


public class MainActivity extends AppCompatActivity {
    private WebView webView;
    private ValueCallback<Uri[]> uploadMessage;
    private static final int FILE_CHOOSER_REQUEST_CODE = 1;

    private void saveToDownloads(String filename, String base64Data) {
        try {
            // Decode base64 to bytes
            byte[] data = android.util.Base64.decode(base64Data, android.util.Base64.DEFAULT);

            // Use DownloadManager to save to Downloads folder
            DownloadManager downloadManager = (DownloadManager) getSystemService(DOWNLOAD_SERVICE);

            // Write data to a temporary file
            File tempFile = File.createTempFile("temp_save", ".tmp", getExternalCacheDir());
            FileOutputStream fos = new FileOutputStream(tempFile);
            fos.write(data);
            fos.close();

            // Create a download request
            DownloadManager.Request request = new DownloadManager.Request(Uri.fromFile(tempFile));
            request.setTitle(filename);
            request.setDescription("GBA Save State");
            request.setVisibleInDownloadsUi(true);
            request.setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED);
            request.setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, filename);

            downloadManager.enqueue(request);
        } catch (Exception e) {
            e.printStackTrace();
            // Optionally show a toast
        }
    }

    @SuppressLint("SetJavaScriptEnabled")
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);

        // 1. Build the asset loader
        final WebViewAssetLoader assetLoader = new WebViewAssetLoader.Builder()
                .addPathHandler("/assets/", new WebViewAssetLoader.AssetsPathHandler(this))
                .build();

        webView = findViewById(R.id.webview);
        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setAllowFileAccess(false);          // Disable file:// – we use custom scheme
        settings.setAllowContentAccess(false);
        settings.setAllowUniversalAccessFromFileURLs(false);
        settings.setAllowFileAccessFromFileURLs(false);
        settings.setLoadWithOverviewMode(true);
        settings.setUseWideViewPort(true);

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.KITKAT) {
            WebView.setWebContentsDebuggingEnabled(true);
        }

        // 2. Set a WebViewClient that uses the asset loader
        webView.setWebViewClient(new WebViewClientCompat() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView view, String url) {
                // Let the WebView load all URLs (including our custom scheme)
                return false;
            }

            @RequiresApi(api = Build.VERSION_CODES.LOLLIPOP)
            @Override
            public android.webkit.WebResourceResponse shouldInterceptRequest(WebView view,
                                                                              android.webkit.WebResourceRequest request) {
                // Delegate to asset loader
                return assetLoader.shouldInterceptRequest(request.getUrl());
            }
        });

        webView.setWebChromeClient(new WebChromeClient() {
            @Override
            public boolean onShowFileChooser(WebView view, ValueCallback<Uri[]> filePathCallback,
                                             FileChooserParams fileChooserParams) {
                uploadMessage = filePathCallback;
                Intent intent = fileChooserParams.createIntent();
                startActivityForResult(intent, FILE_CHOOSER_REQUEST_CODE);
                return true;
            }
        });

        webView.addJavascriptInterface(new WebAppInterface(this), "Android");

        // 3. Load the page using the custom domain
        webView.loadUrl("https://appassets.androidplatform.net/assets/www/index.html");
    }


    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        if (requestCode == FILE_CHOOSER_REQUEST_CODE && uploadMessage != null) {
            uploadMessage.onReceiveValue(WebChromeClient.FileChooserParams.parseResult(resultCode, data));
            uploadMessage = null;
        } else {
            super.onActivityResult(requestCode, resultCode, data);
        }
    }

    public class WebAppInterface {
        Context context;

        WebAppInterface(Context c) {
            context = c;
        }

        @JavascriptInterface
        public void download(final String base64DataUrl, final String filename, final String mimeType) {
            new Thread(new Runnable() {
                @Override
                public void run() {
                    try {
                        // Extract base64 data
                        String base64 = base64DataUrl.substring(base64DataUrl.indexOf(",") + 1);
                        byte[] data = Base64.decode(base64, Base64.DEFAULT);

                        // Use MediaStore to save to public Downloads
                        ContentValues values = new ContentValues();
                        values.put(MediaStore.MediaColumns.DISPLAY_NAME, filename);
                        values.put(MediaStore.MediaColumns.MIME_TYPE, mimeType);
                        values.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS);

                        Uri uri = context.getContentResolver().insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values);
                        if (uri != null) {
                            try (OutputStream out = context.getContentResolver().openOutputStream(uri)) {
                                out.write(data);
                            }
                            // Success
                            runOnUiThread(() -> Toast.makeText(context, "Downloaded: " + filename, Toast.LENGTH_SHORT).show());
                        } else {
                            throw new Exception("Failed to create MediaStore entry");
                        }
                    } catch (final Exception e) {
                        e.printStackTrace();
                        runOnUiThread(() -> Toast.makeText(context, "Download failed: " + e.getMessage(), Toast.LENGTH_SHORT).show());
                    }
                }
            }).start();
        }
    }

  
}