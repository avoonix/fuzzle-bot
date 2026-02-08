<script setup lang="ts">
import { useFetch } from '@vueuse/core';
import { computed } from 'vue';

interface StickerPub {
  id: string,
  set_id: string,
  // similarity: number
}

const url = computed(() => `/api/banned-stickers`)

const { data: stickers, error, execute: refetch } = useFetch(url, { refetch: true, updateDataOnError: true }).json<StickerPub[]>()

</script>

<template>
  <div class="asdf">
    <!-- <h1>This is an ban similar view</h1>
     <v-slider
        v-model="maxSimilarity"
        thumb-label
        min="0.7"
        max="1"
        step="0.005"
      ></v-slider>
    {{ route.params.setId }}
    asdf
    {{ error }}

    <v-btn @click="toggleBan" v-if="isBanned" color="error">
      Unban
    </v-btn>
    <v-btn @click="toggleBan" v-else color="success">
      Ban
    </v-btn> -->


      <div v-if="stickers">
        <div v-for="sticker of stickers">
          <img :src="`/api/banned-sticker/${sticker.id}/thumbnail.png`" loading="lazy" width="128" height="128" />
          {{ sticker }}
        </div>
      </div>
  </div>
</template>

<style>

</style>
